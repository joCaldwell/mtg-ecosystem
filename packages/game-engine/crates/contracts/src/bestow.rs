//! CR 702.103a-e, 207.2a, 205.1a, 208.3. Definition supplies only the
//! printed characteristics; the casting procedure supplies the chosen form.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Nonserialized operands for the supported single-faced enchantment creature.
/// The alternative changes what is paid, never the printed mana cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BestowCastPlan {
    printed: Characteristics,
    bestow_cost: ManaCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestowForm {
    Creature,
    Aura,
}

impl BestowCastPlan {
    pub fn new(printed: &Characteristics, bestow_cost: ManaCost) -> Result<Self, String> {
        if printed.kind != CardKind::Creature
            || printed.additional_types != [CardKind::Enchantment]
            || printed.basic
            || printed.power.is_none()
            || printed.toughness.is_none()
            || printed
                .subtypes
                .iter()
                .any(|s| unsupported_rules_subtype(s) || s == "Aura")
            || printed.mana_cost.generic.is_negative()
            || bestow_cost.generic.is_negative()
        {
            return Err(
                "bestow requires a fixed-cost enchantment creature with ordinary creature subtypes"
                    .into(),
            );
        }
        Ok(Self {
            printed: printed.clone(),
            bestow_cost,
        })
    }
    pub fn base_cost(&self, form: BestowForm) -> &ManaCost {
        match form {
            BestowForm::Creature => &self.printed.mana_cost,
            BestowForm::Aura => &self.bestow_cost,
        }
    }
    pub fn target_domain(&self, form: BestowForm) -> Option<TargetDomain> {
        (form == BestowForm::Aura).then_some(TargetDomain::BattlefieldCreature)
    }
    pub fn characteristics(&self, form: BestowForm) -> Characteristics {
        let mut c = self.printed.clone();
        if form == BestowForm::Aura {
            c.kind = CardKind::Enchantment;
            c.additional_types.clear();
            c.subtypes = vec!["Aura".into()];
            c.power = None;
            c.toughness = None;
        }
        c
    }
    /// A missing announcement is invalid. An announced target which later
    /// becomes illegal makes the spell resolve in its original form.
    pub fn resolving_form(
        &self,
        form: BestowForm,
        legal_targets: &[bool],
    ) -> Result<BestowForm, String> {
        match (form, legal_targets) {
            (BestowForm::Creature, []) | (BestowForm::Aura, [false]) => Ok(BestowForm::Creature),
            (BestowForm::Aura, [true]) => Ok(BestowForm::Aura),
            _ => Err("bestow invocation target count differs from its announced form".into()),
        }
    }
    pub fn detach(&self) -> Characteristics {
        self.characteristics(BestowForm::Creature)
    }
}

#[cfg(test)]
#[test]
fn fixed_bestow_cast_preserves_printed_cost_and_restores_its_creature() {
    let cost = |generic, color| ManaCost {
        generic: Quantity::from(generic),
        colored: vec![color],
    };
    let printed = Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![CardKind::Enchantment],
        subtypes: vec!["Crab".into()],
        basic: false,
        legendary: false,
        snow: false,
        mana_cost: cost(1i64, Color::U),
        colors: vec![Color::U],
        color_identity: vec![Color::U],
        power: Some(2.into()),
        toughness: Some(3.into()),
    };
    let alternative = cost(3i64, Color::U);
    let plan = BestowCastPlan::new(&printed, alternative.clone()).unwrap();
    for (form, paid) in [
        (BestowForm::Creature, &printed.mana_cost),
        (BestowForm::Aura, &alternative),
    ] {
        assert_eq!(plan.base_cost(form), paid);
        assert_eq!(plan.characteristics(form).mana_cost, printed.mana_cost);
    }
    assert_eq!(plan.target_domain(BestowForm::Creature), None);
    assert_eq!(
        plan.target_domain(BestowForm::Aura),
        Some(TargetDomain::BattlefieldCreature)
    );
    assert_eq!(
        plan.resolving_form(BestowForm::Creature, &[]),
        Ok(BestowForm::Creature)
    );
    assert!(plan.resolving_form(BestowForm::Aura, &[]).is_err());
    let attached = plan.characteristics(plan.resolving_form(BestowForm::Aura, &[true]).unwrap());
    assert!(!attached.has_type(CardKind::Creature));
    assert_eq!(attached.subtypes, ["Aura"]);
    assert_eq!((attached.power, attached.toughness), (None, None));
    assert_eq!(
        plan.characteristics(plan.resolving_form(BestowForm::Aura, &[false]).unwrap()),
        printed
    );
    assert_eq!(plan.detach(), printed);
}

pub fn canonical_bestow_program() -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::MainPhase,
        resolution: SpellResolution::Permanent,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: "enchanted-creature".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldCreature,
            filter: None,
        }],
        instructions: vec![],
        modes: vec![],
    }
}

pub fn bestow_alternative(d: &Definition) -> Option<(&ManaCost, &SpellProgram)> {
    d.static_abilities.iter().find_map(|a| match &a.effect {
        StaticEffect::Bestow { cost, program } => Some((cost, program)),
        _ => None,
    })
}

fn ordinary_spell() -> SpellProgram {
    let mut p = canonical_bestow_program();
    p.targets.clear();
    p
}

const SUPPORTED: [KeywordAbility; 6] = {
    use KeywordAbility::*;
    [Flying, FirstStrike, Vigilance, Trample, Lifelink, Reach]
};

fn mana_symbols(cost: &ManaCost) -> Vec<String> {
    let mut symbols = Vec::new();
    if cost.generic != Quantity::zero() || cost.colored.is_empty() {
        symbols.push(cost.generic.to_string());
    }
    symbols.extend(cost.colored.iter().map(|color| format!("{color:?}")));
    symbols
}

fn source_matches(
    d: &Definition,
    cost: &ManaCost,
    power: &Quantity,
    toughness: &Quantity,
    grants: &[KeywordAbility],
) -> Result<bool, String> {
    use serde_json::{Value, json};
    let words = |keys: &[KeywordAbility]| -> Result<Vec<String>, String> {
        if keys.is_empty()
            || keys
                .iter()
                .enumerate()
                .any(|(i, k)| !SUPPORTED.contains(k) || keys[..i].contains(k))
        {
            return Err("unsupported or repeated bestow keyword".into());
        }
        serde_json::from_value(json!(keys)).map_err(|e| e.to_string())
    };
    let intrinsic = words(&d.keywords)?;
    let granted = words(grants)?;
    let positive = |q: &Quantity| {
        q.to_string()
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .map(|n| json!({"sign":1,"amount":{"amount":"fixed","value":n}}))
            .ok_or("invalid enchanted modifier")
    };
    let ast = json!([
        {"kind":"keywords","keywords":[{"keyword":"bestow","cost":[{"cost":"mana","symbols":mana_symbols(cost)}]}]},
        {"kind":"keywords","keywords":intrinsic.iter().map(|k|json!({"keyword":k})).collect::<Vec<_>>()},
        {"kind":"static","effect":{"static":"modify","what":{"ref":"enchanted","noun":"creature"},"modifiers":[
            {"modifier":"pt","power":positive(power)?,"toughness":positive(toughness)?},
            {"modifier":"abilities","abilities":granted.iter().map(|k|json!({"gained":"keyword","keyword":{"keyword":k}})).collect::<Vec<_>>()}]}}
    ]);
    let source: Value = strict_read(&d.provenance.source_json)?;
    let c = &d.characteristics;
    let mut printed = intrinsic.join(", ");
    printed[..1].make_ascii_uppercase();
    let grant_text = match granted.as_slice() {
        [word] => word.clone(),
        [first, last] => format!("{first} and {last}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
        [] => return Ok(false),
    };
    let modifier = format!("Enchanted creature gets +{power}/+{toughness} and has {grant_text}.");
    let mut lines = source["oracle_text"].as_str().unwrap_or_default().lines();
    let bestow = format!("Bestow {}", cost.printed_fixed_cost());
    let line = lines.next().unwrap_or_default();
    if line != bestow
        && !line
            .strip_prefix(&bestow)
            .and_then(|s| s.strip_prefix(" ("))
            .and_then(|s| s.strip_suffix(')'))
            .is_some_and(|s| !s.contains(['(', ')']))
    {
        return Ok(false);
    }
    let inventory: Option<Vec<_>> = source["keywords"].as_array().and_then(|a| {
        a.iter()
            .map(|v| v.as_str().map(str::to_ascii_lowercase))
            .collect()
    });
    let mut expected = intrinsic.clone();
    expected.push("bestow".into());
    expected.sort();
    let Some(mut inventory) = inventory else {
        return Ok(false);
    };
    inventory.sort();
    Ok(strict_read::<Value>(&d.provenance.abilities_json)? == ast
        && inventory == expected
        && lines.eq([printed.as_str(), modifier.as_str()])
        && source["layout"] == "normal"
        && source["name"] == d.name
        && source["oracle_id"] == d.oracle_id
        && source["id"] == d.provenance.printing_id
        && source["type_line"] == format!("Enchantment Creature — {}", c.subtypes.join(" "))
        && !c.subtypes.is_empty()
        && !c.legendary
        && !c.snow
        && source["mana_cost"] == c.mana_cost.printed_fixed_cost()
        && source["colors"] == json!(c.colors)
        && source["color_identity"] == json!(c.color_identity)
        && source["power"].as_str() == c.power.as_ref().map(ToString::to_string).as_deref()
        && source["toughness"].as_str() == c.toughness.as_ref().map(ToString::to_string).as_deref())
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if bestow_alternative(d).is_none()
        && !d.capabilities.contains(&Capability::BestowV1)
        && !d.provenance.abilities_json.contains(r#""bestow""#)
        && !d.provenance.source_json.contains("Bestow {")
    {
        return Ok(());
    }
    let [
        StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect: StaticEffect::Bestow { cost, program },
        },
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect:
                StaticEffect::ModifyCreatures {
                    scope: CreatureScope::Enchanted,
                    power,
                    toughness,
                    keywords,
                    restrictions,
                },
        },
    ] = d.static_abilities.as_slice()
    else {
        return Err("bestow requires its alternative and enchanted modifier".into());
    };
    BestowCastPlan::new(&d.characteristics, cost.clone())?;
    if program != &canonical_bestow_program()
        || !restrictions.is_empty()
        || !source_matches(d, cost, power, toughness, keywords)?
        || d.spell.as_ref() != Some(&ordinary_spell())
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err("unsupported complete bestow definition".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut serde_json::Value) {
    use Capability::*;
    use serde_json::json;
    let caps = [
        NormalLayoutV1,
        BaseCreatureV1,
        CastCreatureV1,
        CastEnchantmentV1,
        CreatureTargetV1,
        AttachmentsV1,
        BestowV1,
    ];
    let mut allowed = caps.to_vec();
    allowed.extend(
        SUPPORTED
            .iter()
            .filter_map(|k| intrinsic_keyword_capability(*k)),
    );
    let keyword_caps = SUPPORTED.iter().map(|word| {
        let cap = intrinsic_keyword_capability(*word).unwrap();
        let present = json!({"contains":{"const":word}});
        json!({"if":{"anyOf":[{"properties":{"keywords":present}},
            {"properties":{"static_abilities":{"contains":{"properties":{"effect":{"type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{"properties":{"keywords":present}}}}}}}}}]},
            "then":{"properties":{"capabilities":{"contains":{"const":cap}}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{"const":cap}}}}}})
    }).collect::<Vec<_>>();
    let words = json!({"minItems":1,"uniqueItems":true,"items":{"enum":SUPPORTED}});
    let alternative = json!({"properties":{"functioning_zone":{"const":"stack"},"effect":{"type":"object","required":["bestow"],"properties":{"bestow":{"properties":{"program":{"const":canonical_bestow_program()},"cost":{"properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"}}}}}}}}});
    let modifier = json!({"properties":{"functioning_zone":{"const":"battlefield"},"effect":{"type":"object","required":["modify_creatures"],"properties":{"modify_creatures":{"properties":{"scope":{"const":"enchanted"},"power":{"pattern":"^[1-9][0-9]*$"},"toughness":{"pattern":"^[1-9][0-9]*$"},"restrictions":{"maxItems":0},"keywords":words}}}}}});
    let marker = json!({"type":"object","required":["effect"],"properties":{"effect":{"type":"object","required":["bestow"]}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":BestowV1}}}},
            {"required":["static_abilities"],"properties":{"static_abilities":{"contains":marker}}},
            {"properties":{"provenance":{"properties":{"abilities_json":{"pattern":"\"bestow\""}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":"Bestow \\{"}}}}}]},
        "then":{"allOf":keyword_caps,"required":["static_abilities","keywords","spell"],"properties":{
            "capabilities":{"allOf":caps.iter().map(|cap|json!({"contains":{"const":cap}})).collect::<Vec<_>>(),"uniqueItems":true,"items":{"enum":allowed}},
            "spell":{"const":ordinary_spell()},"intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
            "keywords":words,"characteristics":{"required":["additional_types","power","toughness"],"properties":{"kind":{"const":"creature"},"additional_types":{"const":["enchantment"]},"subtypes":{"minItems":1},"basic":{"const":false},"legendary":{"const":false},"snow":{"const":false},"power":{"type":"string"},"toughness":{"type":"string"}}},
            "static_abilities":{"minItems":2,"maxItems":2,"prefixItems":[alternative,modifier]}
        }}
    }));
}

#[cfg(test)]
fn keywords() -> Vec<KeywordAbility> {
    use KeywordAbility::*;
    vec![Flying, FirstStrike, Vigilance, Trample, Lifelink]
}

#[cfg(test)]
fn cost(generic: i64) -> ManaCost {
    ManaCost {
        generic: generic.into(),
        colored: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
    }
}

#[cfg(test)]
fn statics() -> Vec<StaticAbility> {
    vec![
        StaticAbility {
            functioning_zone: FunctioningZone::Stack,
            effect: StaticEffect::Bestow {
                cost: cost(2),
                program: canonical_bestow_program(),
            },
        },
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::Enchanted,
                power: 4.into(),
                toughness: 4.into(),
                keywords: keywords(),
                restrictions: vec![],
            },
        },
    ]
}

#[cfg(test)]
fn characteristics() -> Characteristics {
    let colors = vec![Color::B, Color::G, Color::R, Color::U, Color::W];
    Characteristics {
        kind: CardKind::Creature,
        additional_types: vec![CardKind::Enchantment],
        basic: false,
        legendary: false,
        snow: false,
        subtypes: vec!["Manticore".into()],
        mana_cost: cost(0),
        colors: colors.clone(),
        color_identity: colors,
        power: Some(4.into()),
        toughness: Some(4.into()),
    }
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw: serde_json::Value = strict_read(include_str!(
        "../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
    ))
    .unwrap();
    let mut d = hand_cast_cascade::witness();
    d.name = raw["source"]["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracleIdentity"].as_str().unwrap().into();
    d.characteristics = characteristics();
    d.keywords = keywords();
    d.triggered.clear();
    d.static_abilities = statics();
    d.spell = Some(ordinary_spell());
    d.provenance.printing_id = raw["source"]["id"].as_str().unwrap().into();
    #[derive(Deserialize)]
    struct Frozen {
        source: Box<serde_json::value::RawValue>,
        faces: Vec<Face>,
    }
    #[derive(Deserialize)]
    struct Face {
        result: Parsed,
    }
    #[derive(Deserialize)]
    struct Parsed {
        abilities: Box<serde_json::value::RawValue>,
    }
    let frozen: Frozen = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
    ))
    .unwrap();
    d.provenance.source_json = frozen.source.get().into();
    d.provenance.abilities_json = frozen.faces[0].result.abilities.get().into();
    d.provenance.source_sha256 = raw["sourceSha256"].as_str().unwrap().into();
    d.provenance.ast_sha256 = raw["astSha256"].as_str().unwrap().into();
    d.provenance.binding_sha256 = raw["bindingSha256"].as_str().unwrap().into();
    d.capabilities = d.requirements();
    d
}

#[test]
fn reserved_bestow_requires_complete_alternative_and_attachment() {
    let d = witness();
    d.validate().unwrap();
    let mut stripped = d.clone();
    stripped.static_abilities.clear();
    stripped.capabilities = stripped.requirements();
    assert!(stripped.validate().is_err());
    let mut changed = d.clone();
    if let StaticEffect::Bestow { program, .. } = &mut changed.static_abilities[0].effect {
        program.targets.clear();
    }
    changed.capabilities = changed.requirements();
    assert!(changed.validate().is_err());
    let mut missing = d;
    missing.static_abilities.pop();
    missing.capabilities = missing.requirements();
    assert!(missing.validate().is_err());
}
