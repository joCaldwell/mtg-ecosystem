//! Continuous controller/subtype filters in layers 6 and 7c. CR 205.3m,
//! 208.3, 611.3a/b, 613.1f and 613.4c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Subtype {
    Elf,
    Goblin,
    Golem,
    Merfolk,
    Sliver,
    Soldier,
    Vampire,
    Warrior,
    Zombie,
}

impl Subtype {
    const ALL: [Self; 9] = [
        Self::Elf,
        Self::Goblin,
        Self::Golem,
        Self::Merfolk,
        Self::Sliver,
        Self::Soldier,
        Self::Vampire,
        Self::Warrior,
        Self::Zombie,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Elf => "Elf",
            Self::Goblin => "Goblin",
            Self::Golem => "Golem",
            Self::Merfolk => "Merfolk",
            Self::Sliver => "Sliver",
            Self::Soldier => "Soldier",
            Self::Vampire => "Vampire",
            Self::Warrior => "Warrior",
            Self::Zombie => "Zombie",
        }
    }

    fn plural(self) -> String {
        match self {
            Self::Elf => "Elves".into(),
            Self::Merfolk => "Merfolk".into(),
            _ => format!("{}s", self.name()),
        }
    }
}

pub fn keyword_rule(keyword: KeywordAbility) -> Option<&'static str> {
    Some(match keyword {
        KeywordAbility::Deathtouch => "702.2",
        KeywordAbility::DoubleStrike => "702.4",
        KeywordAbility::FirstStrike => "702.7",
        KeywordAbility::Flying => "702.9",
        KeywordAbility::Haste => "702.10",
        KeywordAbility::Lifelink => "702.15",
        KeywordAbility::Reach => "702.17",
        KeywordAbility::Trample => "702.19",
        KeywordAbility::Vigilance => "702.20",
        _ => return None,
    })
}

fn additive_stat(text: &str) -> Option<u32> {
    let text = text.strip_prefix('+')?;
    let x = text.parse::<u32>().ok()?;
    (x.to_string() == text).then_some(x)
}

pub fn printed(line: &str) -> Option<(StaticAbility, Value)> {
    let (subject, tail) = line.strip_suffix('.')?.split_once(" you control ")?;
    let (subject, other) = subject
        .strip_prefix("Other ")
        .map_or((subject, false), |s| (s, true));
    let (subtype, creatures_only) = if let Some(named) = subject.strip_suffix(" creatures") {
        (Subtype::ALL.into_iter().find(|s| s.name() == named)?, true)
    } else {
        (
            Subtype::ALL.into_iter().find(|s| s.plural() == subject)?,
            false,
        )
    };
    let (power, toughness, keywords, modifier) = if let Some(name) = tail.strip_prefix("have ") {
        let keyword: KeywordAbility = serde_json::from_value(json!(name)).ok()?;
        keyword_rule(keyword)?;
        (
            0,
            0,
            vec![keyword],
            json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":name}}]}),
        )
    } else if subtype == Subtype::Golem {
        // Other already reviewed additive stats retain their existing encoding.
        let (p, t) = tail.strip_prefix("get ")?.split_once('/')?;
        let (p, t) = (additive_stat(p)?, additive_stat(t)?);
        if p == 0 && t == 0 {
            return None;
        }
        let signed = |n| json!({"sign":1,"amount":{"amount":"fixed","value":n}});
        (
            p,
            t,
            vec![],
            json!({"modifier":"pt","power":signed(p),"toughness":signed(t)}),
        )
    } else {
        return None;
    };
    let mut filter = json!({"subtypes":[subtype.name()],"control":"you"});
    if creatures_only {
        filter["types"] = json!(["creature"]);
    }
    if other {
        filter["other"] = json!(true);
    }
    let a = StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::ControlledSubtype {
                subtype,
                creatures_only,
                other,
            },
            power: i64::from(power).into(),
            toughness: i64::from(toughness).into(),
            keywords,
            restrictions: vec![],
        },
    };
    Some((
        a,
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":filter},"modifiers":[modifier]}}),
    ))
}

pub(crate) fn has_scope(a: &StaticAbility) -> bool {
    matches!(
        a.effect,
        StaticEffect::ModifyCreatures {
            scope: CreatureScope::ControlledSubtype { .. },
            ..
        }
    )
}

pub(crate) fn validate(d: &Definition) -> Result<(), String> {
    let uses = d.static_abilities.iter().any(has_scope);
    if !uses
        && (d.characteristics.kind != CardKind::Creature
            || ![
                " you control have ",
                "Golems you control get ",
                "Golem creatures you control get ",
                r"\u",
            ]
            .iter()
            .any(|s| d.provenance.source_json.contains(s)))
    {
        return Ok(());
    }
    let source: Value = strict_read(&d.provenance.source_json)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let relevant: Vec<_> = text
        .lines()
        .enumerate()
        .filter_map(|(b, line)| printed(line).map(|(ability, ast)| (b, ability, ast)))
        .collect();
    if !uses && relevant.is_empty() {
        return Ok(());
    }
    let fail = "requires the exact printed controller-relative subtype modifier";
    if d.characteristics.kind != CardKind::Creature
        || source["layout"] != "normal"
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || d.static_abilities.iter().filter(|a| has_scope(a)).count() != relevant.len()
    {
        return Err(fail.into());
    }
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if ast.len() != text.lines().count() {
        return Err(fail.into());
    }
    for (block, ability) in ast
        .iter()
        .enumerate()
        .filter(|(_, a)| a["kind"] == "triggered")
    {
        let source_block = u32::try_from(block).map_err(|_| fail)?;
        if !d.triggered.iter().any(|a| {
            !a.program.instructions.is_empty()
                && a.program
                    .instructions
                    .iter()
                    .all(|i| i.source_block() == source_block)
        }) || ability["trigger"].is_null()
        {
            return Err(fail.into());
        }
    }
    for (block, expected, raw) in relevant {
        let index = ast[..block]
            .iter()
            .filter(|a| a["kind"] == "static")
            .count();
        if ast[block] != raw
            || d.static_abilities.get(index) != Some(&expected)
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.statics.get(index).copied() != u32::try_from(block).ok())
        {
            return Err(fail.into());
        }
    }
    Ok(())
}

pub(crate) fn extend_schema(schema: &mut Value) {
    let effect = json!({"required":["modify_creatures"],"properties":{"modify_creatures":{"required":["scope"],"properties":{
        "scope":{"type":"object","required":["controlled_subtype"]}
    }}}});
    let contains_ability = |predicate| {
        json!({"required":["static_abilities"],"properties":{
            "static_abilities":{"contains":{"required":["effect"],"properties":{"effect":predicate}}}
        }})
    };
    let cap = |cap| json!({"contains":{"const":cap}});
    let keywords: Vec<KeywordAbility> = [
        KeywordAbility::Deathtouch,
        KeywordAbility::DoubleStrike,
        KeywordAbility::FirstStrike,
        KeywordAbility::Flying,
        KeywordAbility::Haste,
        KeywordAbility::Lifelink,
        KeywordAbility::Reach,
        KeywordAbility::Trample,
        KeywordAbility::Vigilance,
    ]
    .into_iter()
    .collect();
    let stats = json!({
        "properties":{"scope":{"properties":{"controlled_subtype":{"properties":{"subtype":{"const":"golem"}}}}},
            "keywords":{"maxItems":0},"power":{"pattern":"^(0|[1-9][0-9]*)$"},"toughness":{"pattern":"^(0|[1-9][0-9]*)$"}},
        "anyOf":[{"properties":{"power":{"pattern":"^[1-9][0-9]*$"}}},{"properties":{"toughness":{"pattern":"^[1-9][0-9]*$"}}}]
    });
    let grant = json!({"properties":{"power":{"const":"0"},"toughness":{"const":"0"},
        "keywords":{"minItems":1,"maxItems":1,"items":{"enum":keywords}}}});
    schema["$defs"]["StaticAbility"].as_object_mut().unwrap().entry("allOf")
        .or_insert_with(|| json!([])).as_array_mut().unwrap().push(json!({
            "if":{"required":["effect"],"properties":{"effect":effect}},
            "then":{"properties":{"functioning_zone":{"const":"battlefield"},"effect":{
                "properties":{"modify_creatures":{"allOf":[{"properties":{"restrictions":{"maxItems":0}}},
                    {"anyOf":[stats,grant]}]}}
            }}}
        }));
    let definitions = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    let filtered = cap(Capability::FilteredStaticModifiersV1);
    definitions.push(json!({
        "if":contains_ability(effect.clone()),
        "then":{"properties":{"capabilities":filtered,"characteristics":{"properties":{"kind":{"const":"creature"}}}}},
        "else":{"properties":{"capabilities":{"not":filtered}}}
    }));
    for k in keywords {
        let mut grants_keyword = effect.clone();
        grants_keyword["properties"]["modify_creatures"]["properties"]["keywords"] =
            json!({"contains":{"const":k}});
        definitions.push(json!({
            "if":contains_ability(grants_keyword),
            "then":{"properties":{"capabilities":cap(intrinsic_keyword_capability(k).unwrap())}}
        }));
    }
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/7fbf6afc-8a16-447b-bdef-b7d0b640f76c.json"
    ))
    .unwrap();
    let source = &row["source"];
    let mut d = super::scoped_creature_stats::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = row["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(&row["faces"][0]["result"]["abilities"]).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.characteristics.colors = vec![Color::U];
    d.characteristics.color_identity = vec![Color::U];
    d.characteristics.subtypes = ["Phyrexian", "Human", "Artificer"]
        .map(str::to_owned)
        .to_vec();
    d.characteristics.power = Some(1.into());
    d.characteristics.toughness = Some(1.into());
    d.characteristics.mana_cost = ManaCost {
        generic: 3.into(),
        colored: vec![Color::U],
    };
    d.static_abilities = vec![
        printed(
            source["oracle_text"]
                .as_str()
                .unwrap()
                .lines()
                .nth(1)
                .unwrap(),
        )
        .unwrap()
        .0,
    ];
    d.triggered = vec![TriggeredAbility {
        death_filter: None,
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        program: SpellProgram {
            timing: SpellTiming::Instant,
            functioning_zone: FunctioningZone::Stack,
            resolution: SpellResolution::Instructions,
            targets: vec![],
            modes: vec![],
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            instructions: vec![Instruction::CreateCreatureTokens {
                count: 1,
                token: CreatureToken {
                    colors: vec![],
                    subtypes: vec!["Phyrexian".into(), "Golem".into()],
                    additional_types: vec![CardKind::Artifact],
                    keywords: vec![],
                    power: 3.into(),
                    toughness: 3.into(),
                },
                source_block: 0,
            }],
        },
    }];
    d.capabilities = d.requirements();
    d
}
