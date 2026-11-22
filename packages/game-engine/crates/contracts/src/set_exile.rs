//! Nontargeted exile, CR 109.2, 400.7, 608.2h, 701.11, 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub struct SetExileClause {
    pub set: ObjectSet,
    pub what: Value,
    pub successors: Value,
}

impl SetExileClause {
    pub fn parse(text: &str) -> Option<Self> {
        let (set, what, successors) = match text {
            "Exile all artifacts." => (
                ObjectSet::Artifacts,
                json!({"ref":"all","filter":{"types":["artifact"]}}),
                json!({"types":["artifact"],"cls":"card"}),
            ),
            "Exile all graveyards." => (
                ObjectSet::GraveyardCards,
                json!({"ref":"zone-cards","zone":"graveyard","owner":{"player":"each-player"}}),
                json!({"cls":"card"}),
            ),
            "Exile all cards from all graveyards." => {
                let filter =
                    json!({"cls":"card","zone":{"zone":"graveyard","owner":"all-players"}});
                (
                    ObjectSet::GraveyardCards,
                    json!({"ref":"all","filter":filter}),
                    filter,
                )
            }
            _ => return None,
        };
        Some(Self {
            set,
            what,
            successors,
        })
    }
    pub fn mode(text: &str) -> Option<Self> {
        if let Some(clause) = Self::parse(text) {
            return Some(clause);
        }
        let (set, kind) = match text {
            "Exile all creatures." => (ObjectSet::Creatures, "creature"),
            "Exile all enchantments." => (ObjectSet::Enchantments, "enchantment"),
            _ => return None,
        };
        Some(Self {
            set,
            what: json!({"ref":"all","filter":{"types":[kind]}}),
            successors: json!({"types":[kind],"cls":"card"}),
        })
    }
    pub fn ability(&self) -> Value {
        json!({"kind":"spell","effects":[{"steps":[{"effect":"exile","what":self.what}]}]})
    }
}

/// Exact printed set operations, complete child ASTs, and the executable root.
pub fn set_exile_modes(
    text: &str,
    timing: SpellTiming,
) -> Option<(Vec<SetExileClause>, Value, SpellProgram)> {
    let mut lines = text.split('\n');
    if lines.next() != Some("Choose one or more —") {
        return None;
    }
    let clauses: Vec<_> = lines
        .map(|l| SetExileClause::mode(l.strip_prefix("• ")?))
        .collect::<Option<_>>()?;
    if clauses.len() < 2 || u32::try_from(clauses.len()).is_err() {
        return None;
    }
    let mut root = SpellProgram {
        timing,
        resolution: SpellResolution::OneOrMoreModes,
        functioning_zone: FunctioningZone::Stack,
        instructions: vec![],
        modes: vec![],
        targets: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    };
    let mut options = vec![];
    for c in &clauses {
        let child = SpellProgram {
            modes: vec![],
            timing,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            additional_sacrifice: None,
            optional_additional_costs: vec![],
            resolution: SpellResolution::Instructions,
            instructions: vec![exile(&c.set)],
        };
        root.modes.push(SpellMode { program: child });
        options.push(json!({"effects":[{"steps":[{"effect":"exile","what":c.what}]}]}));
    }
    let ast = json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"atLeast":1},"options":options}]}]}]);
    Some((clauses, ast, root))
}
fn subset(p: &SpellProgram) -> bool {
    p.resolution == SpellResolution::OneOrMoreModes || p.modes.iter().any(|m| subset(&m.program))
}

/// CR 406.1: exile every member of one closed set.
fn exile(set: &ObjectSet) -> Instruction {
    Instruction::act(
        Verb::Move {
            what: set.kernel(Context::Spell),
            to: Destination::Exile,
        },
        0,
    )
}

fn reserved(p: &SpellProgram) -> bool {
    let selected =
        |set: &ObjectSet| matches!(set, ObjectSet::Artifacts | ObjectSet::GraveyardCards);
    let kernel_sets =
        [ObjectSet::Artifacts, ObjectSet::GraveyardCards].map(|s| s.kernel(Context::Spell));
    p.instructions.iter().any(|i| match i {
        kernel!(Destroy { what, .. }) | kernel!(Move { what, .. }) => kernel_sets.contains(what),
        Instruction::GrantKeywordsSet { set, .. } => selected(set),
        i if ObjectSet::pumped(i).is_some() => ObjectSet::pumped(i).is_some_and(|what| {
            [ObjectSet::Artifacts, ObjectSet::GraveyardCards]
                .iter()
                .any(|set| set.denotes(what))
        }),
        i => i
            .set_damage()
            .is_some_and(|damage| damage.sets.iter().any(|set| kernel_sets.contains(set))),
    }) || p.modes.iter().any(|m| reserved(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let fail = || "set exile requires its complete printed ordinary spell and zone".to_owned();
    if d.activated
        .iter()
        .any(|a| reserved(&a.program) || subset(&a.program))
        || d.static_abilities.iter().any(
            |a| matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if subset(program)),
        )
        || d.triggered
            .iter()
            .any(|a| reserved(&a.program) || subset(&a.program))
    {
        return Err(fail());
    }
    let uses = d.spell.as_ref().is_some_and(|p| reserved(p) || subset(p));
    let raw = &d.provenance.source_json;
    if !uses
        && !raw.contains("Choose one or more")
        && !raw.contains("Exile all ")
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    if d.spell.as_ref().is_some_and(subset) || text.lines().any(|l| l == "Choose one or more —") {
        let (kind, timing) = match d.characteristics.kind {
            CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
            CardKind::Instant => ("Instant", SpellTiming::Instant),
            _ => return Err(fail()),
        };
        let (_, ast, root) = set_exile_modes(text, timing).ok_or_else(fail)?;
        let parsed: Value = strict_read(&d.provenance.abilities_json)?;
        let labels = SourceLabels {
            text: text.into(),
            blocks: vec![text.into()],
            spell_modes: text.lines().skip(1).map(str::to_owned).collect(),
            ..Default::default()
        };
        if ast != parsed
            || d.spell.as_ref() != Some(&root)
            || source["name"] != d.name
            || source["id"] != d.provenance.printing_id
            || source["oracle_id"] != d.oracle_id
            || source["layout"] != "normal"
            || source["type_line"] != kind
            || source["mana_cost"] != d.characteristics.mana_cost.printed_fixed_cost()
            || d.characteristics.basic
            || d.characteristics.legendary
            || !d.keywords.is_empty()
            || !d.triggered.is_empty()
            || !d.activated.is_empty()
            || !d.mana_abilities.is_empty()
            || !d.static_abilities.is_empty()
            || d.x_mana_cost
            || d.intrinsic_zone.is_some()
            || !d.characteristics.additional_types.is_empty()
            || !d.characteristics.subtypes.is_empty()
            || d.characteristics.snow
            || d.source_labels.as_ref().is_some_and(|l| l != &labels)
        {
            return Err(fail());
        }
        return Ok(());
    }
    let Some(clause) = SetExileClause::parse(text) else {
        return if uses
            || text
                .lines()
                .any(|line| SetExileClause::parse(line).is_some())
        {
            Err(fail())
        } else {
            Ok(())
        };
    };
    let (type_line, timing) = match d.characteristics.kind {
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        _ => return Err(fail()),
    };
    let p = d.spell.as_ref().ok_or_else(fail)?;
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    if source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || source["oracle_id"] != d.oracle_id
        || source["layout"] != "normal"
        || source["type_line"] != type_line
        || source["mana_cost"] != d.characteristics.mana_cost.printed_fixed_cost()
        || ast != json!([clause.ability()])
        || d.characteristics.basic
        || d.characteristics.legendary
        || d.characteristics.snow
        || !d.characteristics.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || d.x_mana_cost
        || p.timing != timing
        || p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !p.modes.is_empty()
        || !p.targets.is_empty()
        || p.additional_sacrifice.is_some()
        || !p.optional_additional_costs.is_empty()
        || p.instructions.as_slice() != [exile(&clause.set)]
        || d.source_labels
            .as_ref()
            .is_some_and(|l| l.blocks.as_slice() != [text])
    {
        return Err(fail());
    }
    Ok(())
}

pub(super) fn extend_schema(s: &mut Value) {
    let sets = json!({"enum":[ObjectSet::Artifacts,ObjectSet::GraveyardCards]});
    let exiles = |sets: &[ObjectSet]| json!({"enum":sets.iter().map(exile).collect::<Vec<_>>()});
    let kernel_sets =
        [ObjectSet::Artifacts, ObjectSet::GraveyardCards].map(|s| s.kernel(Context::Spell));
    let free = json!({"$ref":"#/$defs/FixedExileSetFreeProgram"});
    s["$defs"]["FixedExileSetFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":{"anyOf":[
        {"required":["set"],"properties":{"set":sets}},{"required":["sets"],"properties":{"sets":{"contains":sets}}},
        instruction_schema::kernel_set(&kernel_sets)
    ]}}},"modes":{"items":{"properties":{"program":free}}}}});
    let has = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let shape = json!({"required":["spell"],"properties":{"triggered":{"maxItems":0},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},
        "keywords":{"maxItems":0},"static_abilities":{"maxItems":0},"x_mana_cost":{"const":false},
        "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"legendary":{"const":false},"basic":{"const":false},
            "snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},
        "spell":{"type":"object","properties":{"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"targets":{"maxItems":0},
            "modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
            "instructions":{"minItems":1,"maxItems":1,"items":exiles(&[ObjectSet::Artifacts,ObjectSet::GraveyardCards])}}},
        "capabilities":{"contains":{"const":Capability::BoardWipesV1}}}});
    let def = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    def.push(json!({"properties":{"triggered":{"items":{"properties":{"program":free}}},"activated":{"items":{"properties":{"program":free}}}},"if":has,"then":{"anyOf":[shape,{"properties":{"spell":{"properties":{"resolution":{"const":"one_or_more_modes"}}}}}]}}));
    let marker = json!({"type":"object","required":["resolution"],"properties":{"resolution":{"const":"one_or_more_modes"}}});
    let marker_free = json!({"$ref":"#/$defs/SubsetModeFree"});
    s["$defs"]["SubsetModeFree"] = json!({"not":marker,"properties":{"modes":{"items":{"properties":{"program":marker_free}}}}});
    let def = s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap();
    let parent =
        json!({"required":["spell"],"properties":{"spell":{"type":"object","allOf":[marker]}}});
    def.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":marker_free}}},"triggered":{"items":{"properties":{"program":marker_free}}},
        "static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":marker_free}}}}}}}
    },"if":parent,"then":{"properties":{
        "spell":{"properties":{"targets":{"maxItems":0},"instructions":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},
        "modes":{"minItems":2,"items":{"properties":{"program":{"properties":{"resolution":{"const":"instructions"},"targets":{"maxItems":0},"additional_sacrifice":{"type":"null"},
            "optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},"instructions":{"minItems":1,"maxItems":1,"items":exiles(&[ObjectSet::Artifacts,ObjectSet::Creatures,ObjectSet::Enchantments,ObjectSet::GraveyardCards])}}}}}}}},
        "activated":{"maxItems":0},"triggered":{"maxItems":0},"keywords":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},
        "characteristics":{"properties":{"kind":{"enum":["sorcery","instant"]},"legendary":{"const":false},"basic":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0},"subtypes":{"maxItems":0}}},"x_mana_cost":{"const":false},
        "capabilities":{"allOf":[{"contains":{"const":Capability::ModalChoiceV1}},{"contains":{"const":Capability::NonemptyModeSubsetV1}},{"contains":{"const":Capability::BoardWipesV1}}]}
    }},"else":{"properties":{"spell":marker_free,"capabilities":{"not":{"contains":{"const":Capability::NonemptyModeSubsetV1}}}}}}));
    def.push(json!({"if":{"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""oracle_text"\s*:\s*"Choose one or more —"#}}}}},"then":parent}));
    for (pattern, set) in [
        (
            r#""oracle_text"\s*:\s*"Exile all artifacts\.""#,
            ObjectSet::Artifacts,
        ),
        (
            r#""oracle_text"\s*:\s*"Exile all (cards from all )?graveyards\.""#,
            ObjectSet::GraveyardCards,
        ),
    ] {
        def.push(json!({"if":{"properties":{"provenance":{"properties":{"source_json":{"pattern":pattern}}}}},"then":{"allOf":[shape,
            {"properties":{"spell":{"properties":{"instructions":{"items":{"properties":{"set":{"const":set}}}}}}}}]}}));
    }
}
