//! Complete source-bound optional fixed mana kicker, not an effect interpreter.
//! CR 118.8/118.9d, 601.2b/f, 702.33a/d/e, 607.2 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KickedCondition {
    condition: String,
    what: KickedSource,
    comparison: KickedComparison,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct KickedSource {
    #[serde(rename = "ref")]
    reference: String,
    noun: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct KickedComparison {
    op: String,
    value: Count,
}
impl KickedCondition {
    fn supported(&self) -> bool {
        self.condition == "kicked"
            && self.what.reference == "this"
            && self.what.noun == "spell"
            && self.comparison.op == "ge"
            && fixed_one(&self.comparison.value)
    }
}

fn bound_reference(
    binding: &serde_json::Value,
    path: &str,
    expression: serde_json::Value,
    target: &str,
) -> bool {
    let Some(references) = binding["references"].as_array() else {
        return false;
    };
    let references = references
        .iter()
        .filter(|r| r["path"] == path)
        .collect::<Vec<_>>();
    matches!(references.as_slice(), [r] if r["binding"] == "bound" && r["expression"] == expression && r["target"] == target)
}

pub(super) fn declare(
    abilities: &[Ability],
    binding: &serde_json::Value,
    oracle: &str,
    program: &mut SpellProgram,
) -> Result<(), Failure> {
    let mut linked = BTreeSet::new();
    for (block, ability) in abilities.iter().enumerate() {
        let Ability::Keywords { keywords, .. } = ability else {
            continue;
        };
        for (index, keyword) in keywords.iter().enumerate() {
            if keyword.keyword != "kicker" {
                continue;
            }
            if !program.optional_additional_costs.is_empty() || keywords.len() != 1 || index != 0 {
                return Err(unsupported(
                    "keyword.kicker",
                    "multiple/combined kicker declarations unsupported",
                ));
            }
            let Some(costs) = &keyword.cost else {
                return Err(unsupported(
                    "keyword.kicker.cost",
                    "missing fixed mana cost",
                ));
            };
            let [SourceCost::Mana { symbols }] = costs.as_slice() else {
                return Err(unsupported(
                    "keyword.kicker.cost",
                    "unsupported optional cost type",
                ));
            };
            if symbols.is_empty()
                || symbols
                    .iter()
                    .any(|s| s.is_empty() || s.contains(['{', '}']) || s == "X")
            {
                return Err(unsupported(
                    "keyword.kicker.cost",
                    "requires complete fixed mana symbols",
                ));
            }
            let mana = mana_cost(
                &symbols
                    .iter()
                    .map(|s| format!("{{{s}}}"))
                    .collect::<String>(),
            )?;
            let path = format!("/faces/0/abilities/{block}/keywords/{index}");
            let identity = format!("card:{oracle}{path}#keyword");
            let nodes = binding["nodes"]
                .as_array()
                .ok_or_else(|| unsupported("binding", "missing keyword nodes"))?;
            if binding["identity"] != oracle
                || nodes.iter().filter(|n| n["id"] == identity).count() != 1
                || !nodes.iter().any(|n| {
                    n["id"] == identity
                        && n["path"] == path
                        && n["node"] == "keyword-ability"
                        && n["keyword"] == "kicker"
                        && n["scope"] == format!("card:{oracle}/faces/0/abilities/{block}#ability")
                        && n["abilities"] == serde_json::json!(["static"])
                        && n["you"] == format!("card:{oracle}{path}#you")
                })
                || !nodes.iter().any(|n| {
                    n["id"] == format!("card:{oracle}{path}#you")
                        && n["path"] == path
                        && n["node"] == "runtime-player"
                        && n["role"] == "prospective-caster"
                        && n["scope"] == format!("card:{oracle}/faces/0/abilities/{block}#ability")
                })
                || !bound_reference(
                    binding,
                    &format!("{path}/cost/0/payer"),
                    serde_json::json!({"player":"you"}),
                    &format!("card:{oracle}{path}#you"),
                )
            {
                return Err(unsupported(
                    "binding.kicker",
                    "missing exact source keyword binding",
                ));
            }
            program
                .optional_additional_costs
                .push(OptionalAdditionalManaCost {
                    identity,
                    source_block: source_block(block)?,
                    mana,
                });
        }
    }
    for (block, ability) in abilities.iter().enumerate() {
        let Ability::Spell { effects, .. } = ability else {
            continue;
        };
        for (sentence, effect) in effects.iter().enumerate() {
            if effect.condition.is_none() && effect.condition_kind.is_none() {
                continue;
            }
            let [cost] = program.optional_additional_costs.as_slice() else {
                return Err(unsupported(
                    "effects.condition",
                    "missing optional cost declaration",
                ));
            };
            if effect.condition_kind.as_deref() != Some("if")
                || !effect
                    .condition
                    .as_ref()
                    .is_some_and(KickedCondition::supported)
                || !effect.bindings.is_empty()
                || !matches!(effect.steps.as_slice(), [Step::Draw { who, amount:DrawAmount::Fixed { value }, optional:false }]
                    if who.player == "you" && who.of.is_none() && who.selection_id.is_none() && *value > 0)
            {
                return Err(unsupported(
                    "effects.condition",
                    "unsupported complete kicked condition/draw",
                ));
            }
            let path = format!("/faces/0/abilities/{block}/effects/{sentence}/condition");
            let nodes = binding["nodes"]
                .as_array()
                .ok_or_else(|| unsupported("binding", "missing linked nodes"))?;
            let links = nodes
                .iter()
                .filter(|n| n["path"] == path && n["node"] == "linked-ability")
                .collect::<Vec<_>>();
            if links.len() != 1
                || links[0]["id"] != format!("card:{oracle}{path}#linked-ability")
                || links[0]["scope"] != format!("card:{oracle}/faces/0/abilities/{block}#ability")
                || links[0]["condition"] != "kicked"
                || links[0]["ability"] != cost.identity
                || !bound_reference(
                    binding,
                    &format!("{path}/what"),
                    serde_json::json!({"ref":"this","noun":"spell"}),
                    &format!("card:{oracle}/source#object"),
                )
            {
                return Err(unsupported(
                    "binding.kicked",
                    "condition does not link to exact source keyword",
                ));
            }
            linked.insert(cost.identity.clone());
        }
    }
    if program
        .optional_additional_costs
        .iter()
        .any(|c| !linked.contains(&c.identity))
    {
        return Err(unsupported(
            "keyword.kicker",
            "unconsumed optional cost declaration",
        ));
    }
    Ok(())
}
pub(super) fn append(
    program: &mut SpellProgram,
    effects: &[Sentence],
    block: usize,
    slots: &mut BTreeMap<String, u32>,
) -> Result<(), Failure> {
    // Preserve whole-block recognition for ordinary multi-sentence families.
    // Only a declared kicker program needs the linked conditional split below.
    if program.optional_additional_costs.is_empty() {
        return append_spell_effects(program, effects, block, slots, PlayerBinding::Unbound);
    }
    // Each condition was proven complete against its exact source keyword above.
    // Ordinary instructions still use the unchanged shared target binder.
    if effects.is_empty() {
        return Err(unsupported(
            "effects",
            "empty spell effects are unsupported",
        ));
    }
    for sentence in effects {
        if sentence.condition.is_some() {
            let [cost] = program.optional_additional_costs.as_slice() else {
                return Err(unsupported("effects.condition", "missing cost"));
            };
            let [
                Step::Draw {
                    amount: DrawAmount::Fixed { value },
                    ..
                },
            ] = sentence.steps.as_slice()
            else {
                return Err(unsupported(
                    "effects.condition",
                    "unsupported linked effect",
                ));
            };
            program
                .instructions
                .push(Instruction::DrawCardsIfAdditionalCost {
                    cost_identity: cost.identity.clone(),
                    count: Quantity::from(i64::from(*value)),
                    source_block: source_block(block)?,
                });
        } else {
            append_spell_effects(
                program,
                std::slice::from_ref(sentence),
                block,
                slots,
                PlayerBinding::Unbound,
            )?;
        }
    }
    Ok(())
}
pub(super) fn instant_keywords(
    definition: &mut Definition,
    keywords: &[SourceKeyword],
    block: usize,
) -> Result<(), Failure> {
    let [keyword] = keywords else {
        return Err(unsupported(
            "abilities.keywords",
            "unsupported spell keyword combination",
        ));
    };
    // This narrower complete-source instant/sorcery family still admits one
    // direct self instance; permanent keyword multiplicity is independent.
    if keyword.keyword != "cascade"
        || keyword.cost.is_some()
        || keyword.filter.is_some()
        || definition
            .triggered
            .iter()
            .any(|a| a.condition == TriggerCondition::SelfSpellCast)
    {
        return Err(unsupported(
            "abilities.keywords",
            "unsupported instant/sorcery keyword or multiple cascade",
        ));
    }
    definition.triggered.push(TriggeredAbility {
        functioning_zone: FunctioningZone::Stack,
        condition: TriggerCondition::SelfSpellCast,
        intervening_if: None,
        death_filter: None,
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            modes: vec![],
            instructions: vec![Instruction::Cascade {
                threshold: CascadeThreshold::SourceSpellManaValue,
                source_block: source_block(block)?,
            }],
        },
    });
    Ok(())
}
