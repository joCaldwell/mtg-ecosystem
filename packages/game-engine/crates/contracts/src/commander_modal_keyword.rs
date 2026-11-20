//! The commander condition authorizes one additional mode choice at announcement.
//! Each printed mode creates its own continuous effect (CR 601.2b, 608.2c,
//! 611.2c, 700.2a/d, 702.16h, 903.3).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub const TEXT: &str = "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n• Creatures you control gain flying, vigilance, and double strike until end of turn.\n• Creatures you control gain lifelink, indestructible, and protection from each color until end of turn.";

pub fn mode_labels() -> Vec<String> {
    let mut labels: Vec<_> = TEXT.lines().skip(1).map(str::to_owned).collect();
    labels.push(labels.join("\n"));
    labels
}

pub fn abilities() -> Value {
    let keyword = |name| json!({"gained":"keyword","keyword":{"keyword":name}});
    let grant = |abilities: Vec<Value>| {
        json!({"effects":[{"steps":[{"effect":"gain-abilities",
        "what":{"ref":"each","filter":{"types":["creature"],"control":"you"}},
        "abilities":abilities,"duration":{"duration":"end-of-turn"}}]}]})
    };
    let first = grant(vec![
        keyword("flying"),
        keyword("vigilance"),
        keyword("double strike"),
    ]);
    let second = grant(vec![
        keyword("lifelink"),
        keyword("indestructible"),
        json!({"gained":"keyword","keyword":{"keyword":"protection","from":[{"scope":"all-colors"}]}}),
    ]);
    json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":1},
        "conditionalCount":{"condition":{"condition":"control","who":{"player":"you"},
            "what":{"ref":"filter","filter":{"commander":true},"count":{"amount":"fixed","value":1}}},
            "count":{"min":1,"max":2}},"options":[first,second]}]}]}])
}

static PROGRAM: LazyLock<SpellProgram> = LazyLock::new(|| {
    use KeywordAbility::*;
    let grant = |keywords| Instruction::GrantKeywordsSet {
        set: ObjectSet::ControllerCreatures,
        modifier: None,
        keywords,
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    };
    let first = grant(vec![Flying, Vigilance, DoubleStrike]);
    let second = grant(vec![Lifelink, Indestructible, ProtectionFromEachColor]);
    let program = |instructions| SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        instructions,
        targets: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    };
    let mut root = program(vec![]);
    root.modes = [
        vec![first.clone()],
        vec![second.clone()],
        vec![first, second],
    ]
    .map(|instructions| SpellMode {
        program: program(instructions),
    })
    .into();
    root
});

pub fn program() -> &'static SpellProgram {
    &PROGRAM
}

pub fn akromas_will_program(p: &SpellProgram) -> bool {
    p.modes.len() == 3 && p == program()
}

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

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    // Check retained source even if both the protected mode and its capability were erased.
    let raw = &d.provenance.source_json;
    let raw_ast = &d.provenance.abilities_json;
    let admitted = d.spell.as_ref().is_some_and(akromas_will_program);
    let clause = "If you control a commander as you cast this spell";
    if !admitted
        && !raw.contains(clause)
        && !raw.contains(r"\u")
        && !raw_ast.contains("conditionalCount")
        && !raw_ast.contains(r"\u")
    {
        return Ok(());
    }
    let s: Value = strict_read(raw)?;
    let ast: Value = strict_read(raw_ast)?;
    let expected = abilities();
    if !admitted
        && !s["oracle_text"]
            .as_str()
            .is_some_and(|t| t.contains(clause))
        && ast != expected
    {
        return Ok(());
    }
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for symbol in &c.mana_cost.colored {
        cost.push_str(&format!("{{{symbol:?}}}"));
    }
    if !admitted
        || d.capabilities != BTreeSet::from(CAPS)
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
        || s["oracle_text"] != TEXT
        || ast != expected
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
            l.text != TEXT
                || l.blocks != [TEXT]
                || l.spell_modes != mode_labels()
                || !l.activated.is_empty()
                || !l.triggered.is_empty()
                || !l.statics.is_empty()
                || !l.mana.is_empty()
        })
    {
        return Err(
            "requires complete commander modal grant source, labels and ordered spell choices"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    // Base schemas check required fields. Accept serde's explicit defaults as
    // well as omitted defaults, including a null modifier on either grant.
    fn ordered(items: Vec<Value>) -> Value {
        if items.is_empty() {
            return json!({"maxItems":0});
        }
        json!({"minItems":items.len(),"prefixItems":items,"items":false})
    }
    let program_schema = |p: &SpellProgram| {
        let instructions = p.instructions.iter().map(|i| {
            let Instruction::GrantKeywordsSet { keywords, .. } = i else { unreachable!() };
            json!({"properties":{"op":{"const":"grant_keywords_set"},"keywords":{"const":keywords},
                "set":{"const":ObjectSet::ControllerCreatures},"modifier":{"type":"null"},
                "duration":{"const":"until_end_of_turn"},"source_block":{"const":0}}})
        }).collect();
        json!({"type":"object","properties":{"resolution":{"const":"instructions"},"timing":{"const":"instant"},
            "functioning_zone":{"const":"stack"},"targets":{"maxItems":0},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":ordered(instructions)}})
    };
    let modes: Vec<_> = program()
        .modes
        .iter()
        .map(|m| program_schema(&m.program))
        .collect();
    schema["$defs"]["CommanderModalKeywordChild"] = json!({"anyOf":modes});
    let mut root = program_schema(program());
    root["required"] = json!(["modes"]);
    root["properties"]["modes"] = ordered(
        modes
            .into_iter()
            .map(|p| json!({"properties":{"program":p}}))
            .collect(),
    );
    schema["$defs"]["CommanderModalKeywordProgram"] = root;
    let cap = json!({"properties":{"capabilities":{"contains":{"const":Capability::CommanderModalKeywordGrantV1}}}});
    let printed = json!({"properties":{"provenance":{"properties":{"source_json":{
        "pattern":r#""oracle_text"\s*:\s*"[^"]*If you control a commander as you cast this spell"#}}}}});
    let printed_text = TEXT
        .replace('.', r"\.")
        .replace('\n', r"\\n")
        .replace('•', r"(?:•|\\u2022)");
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[cap, printed, {"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"}}}]},
        "then":{"required":["spell","source_labels"],"properties":{
            "spell":{"$ref":"#/$defs/CommanderModalKeywordProgram"},
            "capabilities":{"uniqueItems":true,"minItems":CAPS.len(),"maxItems":CAPS.len(),"items":{"enum":CAPS}},
            "intrinsic_zone":{"type":"null"},"keywords":{"maxItems":0},"x_mana_cost":{"const":false},
            "triggered":{"maxItems":0},"activated":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},
            "characteristics":{"properties":{"kind":{"const":"instant"},"basic":{"const":false},"legendary":{"const":false},
                "snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0},"power":{"type":"null"},"toughness":{"type":"null"}}},
            "provenance":{"properties":{"source_json":{"pattern":format!(r#""oracle_text"\s*:\s*"{printed_text}""#)},
                "abilities_json":{"contentMediaType":"application/json","contentSchema":{"const":abilities()}}}},
            "source_labels":{"type":"object","properties":{"text":{"const":TEXT},"blocks":{"const":[TEXT]},"spell_modes":{"const":mode_labels()},
                "activated":{"maxItems":0},"triggered":{"maxItems":0},"statics":{"maxItems":0},"mana":{"maxItems":0}}}
        }}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    // Different identity and mana cost, with the same printed and bound semantics.
    let mut d = power_filtered_removal::witness();
    let mut source: Value = strict_read(&d.provenance.source_json).unwrap();
    source["oracle_text"] = json!(TEXT);
    d.provenance.source_json = source.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = abilities().to_string();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.source_labels = Some(SourceLabels {
        text: TEXT.into(),
        blocks: vec![TEXT.into()],
        spell_modes: mode_labels(),
        ..Default::default()
    });
    d.spell = Some(program().clone());
    d.capabilities = d.requirements();
    d
}

#[test]
fn commander_modal_keyword_closed_definition() {
    let d = witness();
    d.validate().unwrap();
    assert_eq!(d.requirements(), BTreeSet::from(CAPS));
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
            d.spell = Some(program().modes[0].program.clone());
        },
        |d| {
            d.spell = Some(program().modes[1].program.clone());
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
            let mut a = abilities();
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
