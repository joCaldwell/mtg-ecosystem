//! Bounded ordered ordinary spells. CR 109.5, 115, 601.2c, 608.2b/c,
//! 611.2c, 702.26. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) const SCHEMA: &str = "#/$defs/ComposedOrdinarySpellProgram";

// Slice 1 adapter: a source filter and its existing executable target domain
// describe the same current, announcement-time creature predicate.
#[derive(Clone, Copy)]
enum ObjectFilter {
    Creature,
    ControllerCreature,
}

impl ObjectFilter {
    fn read(raw: &Value) -> Option<Self> {
        if *raw == json!({"types":["creature"]}) {
            Some(Self::Creature)
        } else if *raw == json!({"types":["creature"],"control":"you"}) {
            Some(Self::ControllerCreature)
        } else {
            None
        }
    }

    fn domain(self) -> TargetDomain {
        match self {
            Self::Creature => TargetDomain::BattlefieldCreature,
            Self::ControllerCreature => TargetDomain::ControlledBattlefieldCreature,
        }
    }

    fn printed(self) -> &'static str {
        match self {
            Self::Creature => "target creature",
            Self::ControllerCreature => "target creature you control",
        }
    }
}

/// Cheap prefilter over strict JSON. Escaped spellings still reach the decoder.
pub fn may_have_counter(raw: &str) -> bool {
    raw.contains("\"put-counters\"") || raw.contains(r"\u")
}

/// Recognize the candidate from printed clauses too. An AST that omits a
/// supported rider must never fall back to the standalone counter lowerer.
pub fn printed_candidate(text: &str) -> bool {
    text.split_once(". ").is_some_and(|(first, rest)| {
        first.starts_with("Put ")
            && first.contains(" +1/+1 counter")
            && first.contains(" on target creature")
            && (rest.starts_with("It gains ")
                || rest.starts_with("That creature gains ")
                || rest.starts_with("It phases out.")
                || rest.starts_with("That creature phases out."))
    })
}

pub fn candidate(ast: &Value) -> bool {
    ast.as_array().is_some_and(|blocks| {
        blocks.iter().any(|block| {
            let starts_with_counter = block["effects"][0]["steps"][0]["effect"] == "put-counters";
            starts_with_counter
                && block["effects"].as_array().is_some_and(|sentences| {
                    sentences.iter().any(|sentence| {
                        sentence["steps"].as_array().is_some_and(|steps| {
                            steps.iter().any(|step| {
                                matches!(
                                    step["effect"].as_str(),
                                    Some("gain-abilities" | "phase-out")
                                )
                            })
                        })
                    })
                })
        })
    })
}

pub(crate) fn program(p: &SpellProgram) -> bool {
    if p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || !p.modes.is_empty()
        || !matches!(p.targets.as_slice(), [t]
            if t.slot == 0 && t.chooser == Chooser::Controller
                && t.stage == EvaluationStage::Announcement && t.filter.is_none()
                && !t.selection_id.is_empty()
                && matches!(t.domain, TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature))
    {
        return false;
    }
    let [
        Instruction::PutPlusOneCounters {
            on: CounterRecipient::Target { target_slot: 0 },
            amount,
            source_block: 0,
        },
        tails @ ..,
    ] = p.instructions.as_slice()
    else {
        return false;
    };
    if *amount <= Quantity::zero() {
        return false;
    }
    let mut grant = false;
    let mut life = false;
    for (index, instruction) in tails.iter().enumerate() {
        match instruction {
            Instruction::GrantKeywords {
                target_slot: 0,
                modifier: None,
                keywords,
                duration: Duration::UntilEndOfTurn,
                source_block: 0,
            } if !grant && !life && supported_temporary_keywords(keywords) => grant = true,
            Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount,
                source_block: 0,
            } if grant && !life && !amount.is_negative() => life = true,
            Instruction::PhaseOutTargetPermanent {
                target_slot: 0,
                source_block: 0,
            } if index + 1 == tails.len() && !life => return true,
            _ => return false,
        }
    }
    grant
}

pub(crate) fn grant(p: &SpellProgram) -> Option<TemporaryTargetedKeywordProgram<'_>> {
    if !program(p) {
        return None;
    }
    let Instruction::GrantKeywords { keywords, .. } = p.instructions.get(1)? else {
        return None;
    };
    Some(TemporaryTargetedKeywordProgram {
        modifier: None,
        keywords,
        controller_life: None,
        controller_scry: None,
    })
}

const COUNTER: &str = "put-counters";

/// Consume all clauses of the frozen adapter grammar. The compiler also proves
/// the complete binding graph. Re-running this source check on catalog intake
/// catches missing and reordered instructions and changed operands in schema 1.
pub fn archived_program(text: &str, ast: &Value, timing: SpellTiming) -> Option<SpellProgram> {
    let [block] = ast.as_array()?.as_slice() else {
        return None;
    };
    if block.as_object()?.len() != 2 || block["kind"] != "spell" {
        return None;
    }
    let (printed, reminder) = text
        .split_once(" (")
        .map_or((text, None), |(text, reminder)| (text, Some(reminder)));
    let mut clauses = printed.strip_suffix('.')?.split(". ");
    let effects = block["effects"].as_array()?;
    let steps = effects.iter().map(|sentence| {
        if sentence.as_object()?.len() != 1 {
            return None;
        }
        let [step] = sentence["steps"].as_array()?.as_slice() else {
            return None;
        };
        Some(step)
    });
    let mut steps = steps.collect::<Option<Vec<_>>>()?.into_iter();
    let first = steps.next()?;
    let on = &first["on"];
    let filter = ObjectFilter::read(&on["filter"])?;
    let selection = on["selectionId"].as_str()?;
    let cursor = selection.strip_prefix("line:0:")?.parse::<u32>().ok()?;
    if selection != format!("line:0:{cursor}")
        || *on
            != json!({"ref":"target","filter":on["filter"],
            "count":{"amount":"fixed","value":1},"selectionId":selection})
    {
        return None;
    }
    let count = fixed(&first["count"])?;
    if count == 0
        || *first
            != json!({"effect":COUNTER,"on":on,"counter":{"type":"+1/+1"},"count":first["count"]})
    {
        return None;
    }
    let clause = clauses.next()?.strip_prefix("Put ")?;
    let noun = if count == 1 { "counter" } else { "counters" };
    let target = filter.printed();
    let amount = clause.strip_suffix(&format!(" +1/+1 {noun} on {target}"))?;
    if !((amount == "a" && count == 1) || controller_library::matches_fixed_amount(amount, count)) {
        return None;
    }
    let mut p = SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: selection.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: filter.domain(),
            filter: None,
        }],
        instructions: vec![Instruction::PutPlusOneCounters {
            on: CounterRecipient::Target { target_slot: 0 },
            amount: count.into(),
            source_block: 0,
        }],
        modes: vec![],
    };
    for step in steps {
        p.instructions.push(lower_step(step, clauses.next()?)?);
    }
    (clauses.next().is_none() && program(&p) && valid_reminder(reminder, &p)).then_some(p)
}

fn fixed(value: &Value) -> Option<i64> {
    let n = value["value"].as_i64()?;
    (n >= 0 && *value == json!({"amount":"fixed","value":n})).then_some(n)
}

fn lower_step(step: &Value, clause: &str) -> Option<Instruction> {
    let subject = match &step["what"] {
        what if *what == json!({"ref":"it"}) => Some("It"),
        what if *what == json!({"ref":"that","filter":{"types":["creature"]}}) => {
            Some("That creature")
        }
        _ => None,
    };
    match step["effect"].as_str()? {
        "gain-abilities" => {
            let names = step["abilities"]
                .as_array()?
                .iter()
                .map(|ability| {
                    let keyword = &ability["keyword"]["keyword"];
                    (*ability == json!({"gained":"keyword","keyword":{"keyword":keyword}}))
                        .then(|| keyword.as_str())
                        .flatten()
                })
                .collect::<Option<Vec<_>>>()?;
            let keywords = names
                .iter()
                .map(|name| serde_json::from_value::<KeywordAbility>(json!(name)).ok())
                .collect::<Option<Vec<_>>>()?;
            if *step
                != json!({"effect":"gain-abilities","what":step["what"],
                "abilities":step["abilities"],"duration":{"duration":"end-of-turn"}})
                || !supported_temporary_keywords(&keywords)
                || clause
                    != format!(
                        "{} gains {} until end of turn",
                        subject?,
                        keyword_list(&names)?
                    )
            {
                return None;
            }
            Some(Instruction::GrantKeywords {
                target_slot: 0,
                modifier: None,
                keywords,
                duration: Duration::UntilEndOfTurn,
                source_block: 0,
            })
        }
        "phase-out"
            if clause == format!("{} phases out", subject?)
                && *step == json!({"effect":"phase-out","what":step["what"]}) =>
        {
            Some(Instruction::PhaseOutTargetPermanent {
                target_slot: 0,
                source_block: 0,
            })
        }
        "gain-life" => {
            let n = fixed(&step["amount"])?;
            if *step != json!({"effect":"gain-life","who":{"player":"you"},"amount":step["amount"]})
                || clause != format!("You gain {n} life")
            {
                return None;
            }
            Some(Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: n.into(),
                source_block: 0,
            })
        }
        _ => None,
    }
}

fn keyword_list(names: &[&str]) -> Option<String> {
    Some(match names {
        [] => return None,
        [one] => (*one).into(),
        [first, second] => format!("{first} and {second}"),
        [first @ .., last] => format!("{}, and {last}", first.join(", ")),
    })
}

// CR 207.2a. Accept only reviewed explanatory sentences for present primitives.
fn valid_reminder(reminder: Option<&str>, p: &SpellProgram) -> bool {
    const KEYWORD_REMINDERS: &[(KeywordAbility, &str)] = &[
        (
            KeywordAbility::Hexproof,
            "It can't be the target of spells or abilities your opponents control.",
        ),
        (
            KeywordAbility::Indestructible,
            "Damage and effects that say \"destroy\" don't destroy it.",
        ),
        (KeywordAbility::Reach, "It can block creatures with flying."),
        (
            KeywordAbility::FirstStrike,
            "A creature with first strike deals combat damage before creatures without first strike.",
        ),
    ];
    let Some(reminder) = reminder else {
        return true;
    };
    let Some(reminder) = reminder.strip_suffix(')') else {
        return false;
    };
    if reminder
        == "Treat it and anything attached to it as though they don't exist until its controller's next turn."
    {
        return p
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::PhaseOutTargetPermanent { .. }));
    }
    let Some(TemporaryTargetedKeywordProgram { keywords, .. }) = grant(p) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    reminder.split_inclusive(". ").all(|sentence| {
        KEYWORD_REMINDERS.iter().any(|(keyword, expected)| {
            keywords.contains(keyword) && sentence.trim_end() == *expected && seen.insert(*expected)
        })
    }) && !reminder.is_empty()
}

pub(crate) fn validate(d: &Definition) -> Result<(), String> {
    let uses = d.spell.as_ref().is_some_and(program);
    let raw = &d.provenance.source_json;
    if !uses
        && !may_have_counter(&d.provenance.abilities_json)
        && !raw.contains(" +1/+1 counter")
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let source: Value = strict_read(raw)?;
    if !uses
        && !candidate(&ast)
        && !source["oracle_text"]
            .as_str()
            .is_some_and(printed_candidate)
    {
        return Ok(());
    }
    let fail = || "ordinary composition requires its complete printed and bound source".to_string();
    let c = &d.characteristics;
    let (kind, timing) = match c.kind {
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        _ => return Err(fail()),
    };
    let text = source["oracle_text"].as_str().ok_or_else(fail)?;
    let cost = &c.mana_cost;
    let mut printed_cost = String::new();
    if cost.generic != Quantity::zero() || cost.colored.is_empty() {
        printed_cost.push_str(&format!("{{{}}}", cost.generic));
    }
    for color in &cost.colored {
        printed_cost.push_str(&format!("{{{color:?}}}"));
    }
    if !uses
        || d.spell != archived_program(text, &ast, timing)
        || source["layout"] != "normal"
        || source["type_line"] != kind
        || source["name"] != d.name
        || source["oracle_id"] != d.oracle_id
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != printed_cost
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || c.basic
        || c.legendary
        || c.snow
        || c.power.is_some()
        || c.toughness.is_some()
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || d.intrinsic_zone.is_some()
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err(fail());
    }
    Ok(())
}

pub(crate) fn extend_schema(schema: &mut Value) {
    let counter = json!({"properties":{
        "op":{"const":"put_plus_one_counters"}, "on":{"const":CounterRecipient::Target{target_slot:0}},
        "source_block":{"const":0}}});
    let grant = json!({"properties":{
        "op":{"const":"grant_keywords"}, "target_slot":{"const":0}, "modifier":{"type":"null"},
        "keywords":{"$ref":"#/$defs/TemporaryCreatureKeywords"}, "duration":{"const":"until_end_of_turn"},
        "source_block":{"const":0}}});
    let phase = json!({"properties":{
        "op":{"const":"phase_out_target_permanent"}, "target_slot":{"const":0}, "source_block":{"const":0}}});
    let life = json!({"properties":{
        "op":{"const":"gain_life"}, "recipient":{"const":LifeRecipient::Controller}, "source_block":{"const":0}}});
    schema["$defs"]["ComposedOrdinarySpellProgram"] = json!({"type":"object","required":["targets","instructions"],
    "properties":{
        "optional_additional_costs":{"maxItems":0}, "additional_sacrifice":{"type":"null"},
        "modes":{"maxItems":0}, "resolution":{"const":"instructions"}, "functioning_zone":{"const":"stack"},
        "targets":{"minItems":1,"maxItems":1,"items":{"properties":{
            "slot":{"const":0},"chooser":{"const":"controller"},"stage":{"const":"announcement"},
            "selection_id":{"minLength":1}, "filter":{"type":"null"},
            "domain":{"enum":["battlefield_creature","controlled_battlefield_creature"]}}}},
        "instructions":{"minItems":2,"maxItems":3,"prefixItems":[counter,{"anyOf":[grant.clone(),phase.clone()]}],
            "items":{"anyOf":[life.clone(),phase.clone()]},
            "if":{"contains":grant},"else":{"maxItems":2}}
    }});
    let composed = json!({"required":["spell"],"properties":{"spell":{"$ref":SCHEMA}}});
    let cap = json!({"contains":{"const":Capability::ComposedOrdinarySpellV1}});
    let has_life =
        json!({"properties":{"spell":{"properties":{"instructions":{"contains":life}}}}});
    let has_phase =
        json!({"properties":{"spell":{"properties":{"instructions":{"contains":phase}}}}});
    let plain_target = json!({"properties":{"spell":{"properties":{"targets":{"items":{
        "properties":{"domain":{"const":"battlefield_creature"}}}
    }}}}});
    let definition = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    definition.push(json!({
        "if":composed,
        "then":{"allOf":[
            {"properties":{
                "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"hybrid_mana":{"maxItems":0}}},
                "capabilities":{"allOf":[cap.clone(),{"contains":{"const":Capability::PlusOneCountersV1}}]},
                "x_mana_cost":{"const":false},"activated":{"maxItems":0},"triggered":{"maxItems":0},
                "static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}
            }},
            {"if":plain_target,"then":{"properties":{"capabilities":{"contains":{"const":Capability::CreatureTargetV1}}}},
                "else":{"properties":{"capabilities":{"contains":{"const":Capability::AttachmentsV1}}}}},
            {"if":has_life,"then":{"properties":{"capabilities":{"contains":{"const":Capability::LifeGainV1}}}},
                "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::LifeGainV1}}}}}},
            {"if":has_phase,"then":{"properties":{"capabilities":{"contains":{"const":Capability::TargetedPhasingV1}}}},
                "else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TargetedPhasingV1}}}}}}
        ]},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    use serde_json::value::RawValue;
    use std::collections::BTreeMap;

    type RawFields = BTreeMap<String, Box<RawValue>>;
    let raw: RawFields = strict_read(include_str!(
        "../../../tests/fixtures/82506e51-3fe1-446f-83c9-69e67583cefc.json"
    ))
    .unwrap();
    let source_json = raw["source"].get();
    let source: Value = strict_read(source_json).unwrap();
    let faces: Vec<RawFields> = strict_read(raw["faces"].get()).unwrap();
    let result: RawFields = strict_read(faces[0]["result"].get()).unwrap();
    let abilities_json = result["abilities"].get();
    let ast = strict_read(abilities_json).unwrap();
    let archive = accepted_manifest(include_str!(
        "../../../tests/fixtures/archive-manifest-current.json"
    ))
    .unwrap();

    let mut d = power_filtered_removal::witness();
    d.source_labels = None;
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.characteristics.mana_cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![Color::U],
    };
    d.characteristics.colors = vec![Color::U];
    d.characteristics.color_identity = vec![Color::U];
    d.spell = archived_program(
        source["oracle_text"].as_str().unwrap(),
        &ast,
        SpellTiming::Instant,
    );
    d.provenance = SourceProvenance {
        printing_id: source["id"].as_str().unwrap().into(),
        source_index: strict_read(raw["sourceIndex"].get()).unwrap(),
        source_sha256: strict_read(raw["sourceSha256"].get()).unwrap(),
        ast_sha256: strict_read(raw["astSha256"].get()).unwrap(),
        binding_sha256: strict_read(raw["bindingSha256"].get()).unwrap(),
        archive_manifest_sha256: archive.manifest_sha256.clone(),
        parser_sha256: archive.parser_source_sha256.clone(),
        source_json: source_json.into(),
        abilities_json: abilities_json.into(),
    };
    d.capabilities = d.requirements();
    d
}
