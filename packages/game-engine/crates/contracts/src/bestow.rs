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

// The admitted three-block family is the frozen fixed-cost, five-keyword form.
fn keywords() -> Vec<KeywordAbility> {
    use KeywordAbility::*;
    vec![Flying, FirstStrike, Vigilance, Trample, Lifelink]
}

fn cost(generic: i64) -> ManaCost {
    ManaCost {
        generic: generic.into(),
        colored: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
    }
}

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

fn ordinary_spell() -> SpellProgram {
    let mut p = canonical_bestow_program();
    p.targets.clear();
    p
}

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

// These digests fence the exact frozen source and parser graph admitted by this family.
const SOURCE: &str = "0b1a7e4c251b08dc51ea427f0b3cd53c9ed0b4a175e98485a401406f9efd3bcf";
const AST: &str = "cce87e357c185f36c100806cdc98918932499e12f476ac2b3cbba2a9aa164398";
const BINDING: &str = "f1b389dbd4e07d437cd4de25ff4e6cb3a41c9465e53a54e36d08919258226d7b";

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if bestow_alternative(d).is_none()
        && !d.capabilities.contains(&Capability::BestowV1)
        && !d.provenance.abilities_json.contains(r#""bestow""#)
        && !d.provenance.source_json.contains("Bestow {")
    {
        return Ok(());
    }
    if d.provenance.source_sha256 != SOURCE
        || d.provenance.ast_sha256 != AST
        || d.provenance.binding_sha256 != BINDING
        || d.static_abilities != statics()
        || d.keywords != keywords()
        || d.characteristics != characteristics()
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
        FlyingV1,
        FirstStrikeV1,
        VigilanceV1,
        TrampleV1,
        LifelinkV1,
    ];
    let marker = json!({"type":"object","required":["effect"],"properties":{"effect":{"type":"object","required":["bestow"]}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":BestowV1}}}},
            {"required":["static_abilities"],"properties":{"static_abilities":{"contains":marker}}},
            {"properties":{"provenance":{"properties":{"abilities_json":{"pattern":"\"bestow\""}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":"Bestow \\{"}}}}}]},
        "then":{"required":["static_abilities","keywords","spell"],"properties":{
            "capabilities":{"minItems":caps.len(),"maxItems":caps.len(),"uniqueItems":true,"items":{"enum":caps}},
            "spell":{"const":ordinary_spell()},"intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},
            "activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"triggered":{"maxItems":0},
            "provenance":{"properties":{"source_sha256":{"const":SOURCE},"ast_sha256":{"const":AST},"binding_sha256":{"const":BINDING}}},"keywords":{"const":keywords()},"characteristics":{"const":characteristics()},"static_abilities":{"const":statics()}
        }}
    }));
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
    d.provenance.source_sha256 = SOURCE.into();
    d.provenance.ast_sha256 = AST.into();
    d.provenance.binding_sha256 = BINDING.into();
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
