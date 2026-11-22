//! "Choose one. If you control a commander as you cast this spell, you may
//! choose both instead." The commander condition authorizes one additional
//! mode choice at announcement. Each printed mode creates its own continuous
//! effect (CR 601.2b, 608.2c, 611.2c, 700.2a/d, 702.16h, 903.3).
//!
//! The executable shape is structural: three modes, the first two each a
//! grant of keywords to the creatures you control until end of turn and the
//! third both of them in printed order. The printed text, AST, labels and
//! capabilities are derived from those keywords, never from a card.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

/// The printed modal header (CR 700.2a, 903.3).
pub const HEADER: &str =
    "Choose one. If you control a commander as you cast this spell, you may choose both instead.";

/// The Oracle wording, AST form and rule of a keyword a mode may grant.
fn keyword(k: KeywordAbility) -> Option<(&'static str, Value, &'static str)> {
    let plain = |name: &'static str, rule| Some((name, json!({"keyword": name}), rule));
    match k {
        KeywordAbility::Flying => plain("flying", "702.9"),
        KeywordAbility::Vigilance => plain("vigilance", "702.20"),
        KeywordAbility::DoubleStrike => plain("double strike", "702.4"),
        KeywordAbility::FirstStrike => plain("first strike", "702.7"),
        KeywordAbility::Lifelink => plain("lifelink", "702.15"),
        KeywordAbility::Indestructible => plain("indestructible", "702.12"),
        KeywordAbility::Trample => plain("trample", "702.19"),
        KeywordAbility::Haste => plain("haste", "702.10"),
        KeywordAbility::Deathtouch => plain("deathtouch", "702.2"),
        KeywordAbility::ProtectionFromEachColor => Some((
            "protection from each color",
            json!({"keyword":"protection","from":[{"scope":"all-colors"}]}),
            "702.16",
        )),
        _ => None,
    }
}

/// CR 611.2c: one mode's grant to the creatures you control.
fn grant(keywords: Vec<KeywordAbility>) -> Instruction {
    Instruction::GrantKeywordsSet {
        set: ObjectSet::ControllerCreatures,
        modifier: None,
        keywords,
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    }
}

fn mode(instructions: Vec<Instruction>) -> SpellProgram {
    SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        instructions,
        targets: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

/// The modal program granting `first` or `second`, or both with a commander.
pub fn program(first: Vec<KeywordAbility>, second: Vec<KeywordAbility>) -> SpellProgram {
    let (a, b) = (grant(first), grant(second));
    let mut root = mode(vec![]);
    root.modes = [vec![a.clone()], vec![b.clone()], vec![a, b]]
        .map(|instructions| SpellMode {
            program: mode(instructions),
        })
        .into();
    root
}

/// The keyword lists of the two printed modes when `p` is exactly this shape.
pub fn mode_keywords(p: &SpellProgram) -> Option<[&[KeywordAbility]; 2]> {
    fn keywords(m: &SpellMode) -> Option<&[KeywordAbility]> {
        match m.program.instructions.as_slice() {
            [
                Instruction::GrantKeywordsSet {
                    set: ObjectSet::ControllerCreatures,
                    modifier: None,
                    keywords,
                    duration: Duration::UntilEndOfTurn,
                    source_block: 0,
                },
            ] if !keywords.is_empty() && keywords.iter().all(|k| keyword(*k).is_some()) => {
                Some(keywords.as_slice())
            }
            _ => None,
        }
    }
    let [a, b, _] = p.modes.as_slice() else {
        return None;
    };
    let pair = [keywords(a)?, keywords(b)?];
    (*p == program(pair[0].to_vec(), pair[1].to_vec())).then_some(pair)
}

/// CR 700.2, 903.3: a modal spell whose commander condition allows its
/// third mode, both printed modes in order.
pub fn commander_choose_both(p: &SpellProgram) -> bool {
    mode_keywords(p).is_some()
}

fn listed(keywords: &[KeywordAbility]) -> String {
    let names: Vec<_> = keywords
        .iter()
        .filter_map(|k| keyword(*k))
        .map(|k| k.0)
        .collect();
    match names.as_slice() {
        [one] => (*one).to_owned(),
        [a, b] => format!("{a} and {b}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
        [] => String::new(),
    }
}

/// The printed bullets of the two modes.
fn bullets(pair: [&[KeywordAbility]; 2]) -> [String; 2] {
    pair.map(|k| {
        format!(
            "• Creatures you control gain {} until end of turn.",
            listed(k)
        )
    })
}

/// The complete Oracle text of the program's modes.
pub fn text(p: &SpellProgram) -> Option<String> {
    let [a, b] = bullets(mode_keywords(p)?);
    Some(format!("{HEADER}\n{a}\n{b}"))
}

/// The source labels of the three modes: each bullet, then both.
pub fn mode_labels(p: &SpellProgram) -> Option<Vec<String>> {
    let [a, b] = bullets(mode_keywords(p)?);
    let both = format!("{a}\n{b}");
    Some(vec![a, b, both])
}

/// The complete typed AST of the program's text.
pub fn abilities(p: &SpellProgram) -> Option<Value> {
    let option = |keywords: &[KeywordAbility]| {
        let gained: Vec<_> = keywords
            .iter()
            .filter_map(|k| keyword(*k))
            .map(|(_, ast, _)| json!({"gained":"keyword","keyword":ast}))
            .collect();
        json!({"effects":[{"steps":[{"effect":"gain-abilities",
            "what":{"ref":"each","filter":{"types":["creature"],"control":"you"}},
            "abilities":gained,"duration":{"duration":"end-of-turn"}}]}]})
    };
    let [a, b] = mode_keywords(p)?;
    Some(
        json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":1},
        "conditionalCount":{"condition":{"condition":"control","who":{"player":"you"},
            "what":{"ref":"filter","filter":{"commander":true},"count":{"amount":"fixed","value":1}}},
            "count":{"min":1,"max":2}},"options":[option(a),option(b)]}]}]}]),
    )
}

/// The rule of each keyword the two modes grant, in printed order.
pub fn keyword_rules(p: &SpellProgram) -> Option<[Vec<&'static str>; 2]> {
    Some(mode_keywords(p)?.map(|k| k.iter().filter_map(|k| keyword(*k)).map(|k| k.2).collect()))
}

/// Reads the two printed modes' keywords back from the AST, or `None` when
/// the AST is not this shape; `abilities` then proves the whole AST.
pub fn keywords_of(ast: &Value) -> Option<[Vec<KeywordAbility>; 2]> {
    let options = ast[0]["effects"][0]["steps"][0]["options"].as_array()?;
    let [a, b] = options.as_slice() else {
        return None;
    };
    let read = |option: &Value| {
        option["effects"][0]["steps"][0]["abilities"]
            .as_array()?
            .iter()
            .map(|gained| {
                ALL_KEYWORDS
                    .into_iter()
                    .find(|k| keyword(*k).is_some_and(|(_, form, _)| gained["keyword"] == form))
            })
            .collect::<Option<Vec<_>>>()
    };
    let pair = [read(a)?, read(b)?];
    (abilities(&program(pair[0].clone(), pair[1].clone())).as_ref() == Some(ast)).then_some(pair)
}

const ALL_KEYWORDS: [KeywordAbility; 10] = {
    use KeywordAbility::*;
    [
        Flying,
        Vigilance,
        DoubleStrike,
        FirstStrike,
        Lifelink,
        Indestructible,
        Trample,
        Haste,
        Deathtouch,
        ProtectionFromEachColor,
    ]
};

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // Check retained source even if both the protected mode and its capability were erased.
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let admitted = d.spell.as_ref().filter(|p| commander_choose_both(p));
    let clause = "If you control a commander as you cast this spell";
    if admitted.is_none()
        && !raw.contains(clause)
        && !raw.contains(r"\u")
        && !raw_ast.contains("conditionalCount")
        && !raw_ast.contains(r"\u")
        && !d
            .capabilities
            .contains(&Capability::CommanderModalKeywordGrantV1)
    {
        return Ok(());
    }
    let s: Value = strict_read(raw)?;
    let ast: Value = strict_read(raw_ast)?;
    if admitted.is_none()
        && !s["oracle_text"]
            .as_str()
            .is_some_and(|t| t.contains(clause))
        && keywords_of(&ast).is_none()
        && !d
            .capabilities
            .contains(&Capability::CommanderModalKeywordGrantV1)
    {
        return Ok(());
    }
    let fail = || {
        "requires complete commander modal grant source, labels and ordered spell choices"
            .to_owned()
    };
    let p = admitted.ok_or_else(fail)?;
    let text = text(p).ok_or_else(fail)?;
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for symbol in &c.mana_cost.colored {
        cost.push_str(&format!("{{{symbol:?}}}"));
    }
    if !d
        .capabilities
        .contains(&Capability::CommanderModalKeywordGrantV1)
        || c.kind != CardKind::Instant
        || c.legendary
        || c.snow
        || c.basic
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || c.power.is_some()
        || c.toughness.is_some()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.keywords.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.activated.is_empty()
        || !d.mana_abilities.is_empty()
        || s["oracle_text"] != text.as_str()
        || abilities(p).as_ref() != Some(&ast)
        || s["type_line"] != "Instant"
        || s["layout"] != "normal"
        || s["keywords"] != json!([])
        || s["name"] != d.name
        || s["id"] != d.provenance.printing_id
        || s["oracle_id"] != d.oracle_id
        || s["colors"] != json!(c.colors)
        || s["color_identity"] != json!(c.color_identity)
        || s["mana_cost"] != cost
        || ["card_faces", "power", "toughness", "defense", "loyalty"]
            .iter()
            .any(|k| s.get(*k).is_some())
        || d.source_labels.as_ref().is_none_or(|l| {
            l.text != text
                || l.blocks != [text.clone()]
                || Some(&l.spell_modes) != mode_labels(p).as_ref()
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.statics.is_empty()
                || !l.mana.is_empty()
        })
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    // Base schemas check required fields. Accept serde's explicit defaults as
    // well as omitted defaults, including a null modifier on either grant.
    let keywords: Vec<_> = ALL_KEYWORDS.to_vec();
    let grant = json!({"properties":{"op":{"const":"grant_keywords_set"},
        "keywords":{"minItems":1,"items":{"enum":keywords}},
        "set":{"const":ObjectSet::ControllerCreatures},"modifier":{"type":"null"},
        "duration":{"const":"until_end_of_turn"},"source_block":{"const":0}}});
    let program_schema = |count: usize| {
        json!({"type":"object","properties":{"resolution":{"const":"instructions"},"timing":{"const":"instant"},
            "functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":count,"maxItems":count,"items":grant}}})
    };
    schema["$defs"]["CommanderModalKeywordChild"] =
        json!({"anyOf":[program_schema(1), program_schema(2)]});
    let mut root = program_schema(0);
    root["required"] = json!(["modes"]);
    root["properties"]["modes"] = json!({"minItems":3,"maxItems":3,"prefixItems":[
        {"properties":{"program":program_schema(1)}},
        {"properties":{"program":program_schema(1)}},
        {"properties":{"program":program_schema(2)}}
    ],"items":false});
    schema["$defs"]["CommanderModalKeywordProgram"] = root;
    let cap = json!({"properties":{"capabilities":{"contains":{"const":Capability::CommanderModalKeywordGrantV1}}}});
    let printed = json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"[^"]*If you control a commander as you cast this spell"#}}}}});
    let bullet = r"(?:•|\\u2022) Creatures you control gain [a-z ,]+ until end of turn\.";
    let printed_text = format!(r"{}\\n{bullet}\\n{bullet}", HEADER.replace('.', r"\."));
    let base = [
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::ModalChoiceV1,
        Capability::CommanderModalKeywordGrantV1,
        Capability::BoardWipesV1,
        Capability::TemporaryCreatureSetKeywordProgramV1,
    ];
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[cap, printed, {"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"}}}]},
        "then":{"required":["spell","source_labels"],"properties":{
            "spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"},
            "capabilities":{"uniqueItems":true,"allOf":base.iter().map(|c| json!({"contains":{"const":c}})).collect::<Vec<_>>()},
            "intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},"x_mana_cost":{"const":false},
            "triggered":{"maxItems":0},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":"instant"},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"power":{"type":"null"},"toughness":{"type":"null"}}},
            "provenance":{"properties":{"source_json":{"pattern":format!(r#""oracle_text"\s*:\s*"{printed_text}""#)},
                "abilities_json":{"contentMediaType":"application/json"}}},
            "source_labels":{"type":"object","properties":{"blocks":{"minItems":1,"maxItems":1},"spell_modes":{"minItems":3,"maxItems":3},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"statics":{"maxItems":0},"mana":{"maxItems":0}}}
        }}
    }));
}

/// The keywords of the reviewed Akroma's Will printing (a witness only).
#[cfg(test)]
fn witness_program() -> SpellProgram {
    use KeywordAbility::*;
    program(
        vec![Flying, Vigilance, DoubleStrike],
        vec![Lifelink, Indestructible, ProtectionFromEachColor],
    )
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    // Different identity and mana cost, with the same printed and bound semantics.
    let mut d = power_filtered_removal::witness();
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    let p = witness_program();
    let text = text(&p).unwrap();
    source["oracle_text"] = json!(text);
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities(&p).unwrap().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.source_labels = Some(SourceLabels {
        text: text.clone(),
        blocks: vec![text],
        spell_modes: mode_labels(&p).unwrap(),
        ..Default::default()
    });
    d.spell = Some(p);
    d.capabilities = d.requirements();
    d
}

/// The witness's derived capabilities.
#[cfg(test)]
const CAPS: [Capability; 11] = {
    use Capability::*;
    [
        NormalLayoutV1,
        CastInstantV1,
        ModalChoiceV1,
        CommanderModalKeywordGrantV1,
        BoardWipesV1,
        TemporaryCreatureSetKeywordProgramV1,
        FlyingV1,
        VigilanceV1,
        DoubleStrikeV1,
        LifelinkV1,
        IndestructibleV1,
    ]
};

#[test]
fn commander_modal_keyword_closed_definition() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.requirements(), BTreeSet::from(CAPS));
    // The shape, not the card: other keyword pairs derive their own source.
    let other = program(vec![KeywordAbility::Trample], vec![KeywordAbility::Haste]);
    assert!(commander_choose_both(&other));
    assert_eq!(
        text(&other).unwrap(),
        format!(
            "{HEADER}\n• Creatures you control gain trample until end of turn.\n• Creatures you control gain haste until end of turn."
        )
    );
    assert_eq!(
        keywords_of(&abilities(&other).unwrap()).unwrap()[1],
        [KeywordAbility::Haste]
    );
    assert_eq!(
        strict_read::<Definition>(&canonical(&d).unwrap()).unwrap(),
        d
    );
    let protection = KeywordAbility::ProtectionFromEachColor;
    assert_eq!(json!(protection), "protection_from_each_color");
    assert!(!supported_temporary_keywords(&[protection]));
    for independent in [
        temporary_creature_keyword_tests::grant(&[protection], false),
        creature_set_keyword_tests::grant(ObjectSet::ControllerCreatures, &[protection], false),
        sacrifice_set_tests::grant(&[protection]),
    ] {
        assert!(independent.validate().is_err());
    }
    let mut intrinsic = tap_source_destroy_tests::tap_source_destroy();
    intrinsic.keywords.push(protection);
    intrinsic.capabilities = intrinsic.requirements();
    assert!(intrinsic.validate().is_err());
    let token = CreatureToken {
        additional_types: vec![],
        subtypes: vec!["Soldier".into()],
        colors: vec![Color::W],
        power: 1.into(),
        toughness: 1.into(),
        keywords: vec![protection],
    };
    assert!(token.validate().is_err());
    let modification = StaticEffect::ModifyCreatures {
        scope: CreatureScope::Controller,
        power: 0.into(),
        toughness: 0.into(),
        keywords: vec![protection],
        restrictions: vec![],
    };
    assert!(!static_creature_modification_valid(
        CardKind::Enchantment,
        false,
        false,
        &modification
    ));
    let changes: &[fn(&mut Definition)] = &[
        |d| {
            d.spell.as_mut().unwrap().modes[2]
                .program
                .instructions
                .swap(0, 1)
        },
        |d| {
            d.spell.as_mut().unwrap().modes.pop();
        },
        |d| {
            d.spell = Some(witness_program().modes[0].program.clone());
        },
        |d| {
            d.spell = Some(witness_program().modes[1].program.clone());
        },
        |d| {
            d.spell.as_mut().unwrap().modes[1]
                .program
                .instructions
                .clear();
        },
        |d| {
            d.spell.as_mut().unwrap().modes[2].program.timing = SpellTiming::MainPhase;
        },
        |d| d.source_labels = None,
        |d| d.source_labels.as_mut().unwrap().spell_modes.swap(0, 1),
        |d| d.keywords.push(KeywordAbility::ProtectionFromEachColor),
        |d| {
            let mut a = abilities(&witness_program()).unwrap();
            a[0]["effects"][0]["steps"][0]["conditionalCount"]["count"]["max"] = json!(3);
            d.provenance.abilities_json = a.to_string();
            d.provenance.ast_sha256 =
                sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut invalid = d.clone();
        change(&mut invalid);
        invalid.capabilities = invalid.requirements();
        assert!(invalid.validate().is_err(), "mutation {index}");
    }
    for cap in CAPS {
        let mut invalid = d.clone();
        invalid.capabilities.remove(&cap);
        assert!(invalid.validate().is_err(), "{cap:?}");
    }
}
