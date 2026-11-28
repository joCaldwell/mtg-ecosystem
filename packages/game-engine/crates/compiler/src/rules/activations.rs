//! Activated and mana abilities and their costs (CR 602, 605).
//!
//! CR 605.1a: an activated ability without targets that could add mana as it
//! resolves is a mana ability. Its production, costs and the instructions it
//! performs after adding mana ("riders") lower here; riders are Kernel nodes
//! that `kir::mana_rider` admits. CR 106.1, 106.7, 118.3, 119.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const ALL_COLORS: [Color; 5] = [Color::W, Color::U, Color::B, Color::R, Color::G];

fn fixed_amount(value: &serde_json::Value) -> Result<Quantity, Failure> {
    fixed_positive_amount(&typed(value, "amount")?)
}

/// A mana ability's costs and its two-color hybrid symbols (CR 107.4e).
pub(crate) fn lower_permanent_costs(
    costs: &[SourceCost],
    kind: CardKind,
    is_self: &dyn Fn(&str) -> bool,
) -> Result<(PermanentActivationCosts, Vec<ColorHybridSymbol>), Failure> {
    let mut hybrid = vec![];
    let mut result = PermanentActivationCosts {
        remove_source_counters: None,
        tap: false,
        mana: None,
        sacrifice: false,
        pay_life: None,
    };
    for cost in costs {
        match cost {
            SourceCost::RemoveCounters { .. } if result.remove_source_counters.is_none() => {
                result.remove_source_counters = Some(lower_remove_counter_cost(cost, is_self)?);
            }
            SourceCost::TapSelf {} if !result.tap => result.tap = true,
            SourceCost::Mana { symbols } if result.mana.is_none() && hybrid.is_empty() => {
                let m = activation_mana(symbols, "cost")?;
                if m.base.colored.contains(&Color::C) {
                    return Err(unsupported("cost", "colorless-specific costs excluded"));
                }
                // CR 107.3, 107.4f: a mana ability announces no X, and its
                // Phyrexian life payment is not admitted.
                if m.x {
                    return Err(unsupported("cost", "mana ability with X"));
                }
                if m.hybrid
                    .iter()
                    .any(|h| h.colors.contains(&HybridPayment::PayTwoLife))
                {
                    return Err(unsupported("cost", "mana ability with Phyrexian mana"));
                }
                hybrid = m.hybrid;
                // A cost of hybrid symbols alone has no fixed part.
                if m.base != zero_cost() || hybrid.is_empty() {
                    result.mana = Some(m.base);
                }
            }
            // CR 118.3, 119.4: a fixed positive life payment.
            SourceCost::PayLife { amount } if result.pay_life.is_none() => {
                result.pay_life = Some(fixed_amount(amount)?);
            }
            SourceCost::Sacrifice { what }
                if !result.sacrifice
                    && *what
                        == serde_json::json!({"ref":"this","noun":match kind {
                            CardKind::Artifact => "artifact",
                            CardKind::Enchantment => "enchantment",
                            CardKind::Land => "land",
                            _ => "creature",
                        }}) =>
            {
                result.sacrifice = true
            }
            _ => {
                return Err(unsupported(
                    "cost",
                    "unsupported complete permanent activation costs",
                ));
            }
        }
    }
    Ok((result, hybrid))
}

/// CR 602.1a: the cost names the source and a positive fixed number.
pub(crate) fn lower_remove_counter_cost(
    cost: &SourceCost,
    is_self: &dyn Fn(&str) -> bool,
) -> Result<RemoveCounterCost, Failure> {
    if let SourceCost::RemoveCounters {
        counter,
        count,
        from,
    } = cost
        && (from == &serde_json::json!({"ref":"self"})
            || from.as_object().is_some_and(|o| o.len() == 2)
                && from["ref"] == "this"
                && from["noun"].as_str().is_some_and(is_self))
        && count.amount == "fixed"
        && count.value > 0
    {
        return Ok(RemoveCounterCost {
            kind: crate::operands::object_counter_kind(&counter.r#type)?,
            amount: Quantity::from(i64::from(count.value)),
        });
    }
    Err(unsupported("cost", "unsupported source counter removal"))
}

fn zero_cost() -> ManaCost {
    ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    }
}

fn collect_bundles(
    colors: &[Color],
    remaining: u32,
    bundle: &mut Vec<Color>,
    options: &mut Vec<Vec<Color>>,
) {
    if remaining == 0 {
        options.push(bundle.clone());
        return;
    }
    for (i, &color) in colors.iter().enumerate() {
        bundle.push(color);
        collect_bundles(&colors[i..], remaining - 1, bundle, options);
        bundle.pop();
    }
}

/// CR 106.4, 608.2h: a repeated bundle of one mana type counts the set once.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(super) fn counted_mana(
    mana: &SourceMana,
    for_each: &serde_json::Value,
    context: kir::Context,
) -> Result<(Color, kir::Amount), Failure> {
    if let SourceMana::Fixed { symbols } = mana
        && let Some(&color) = symbols.first()
        && symbols.iter().all(|symbol| *symbol == color)
    {
        let value = i64::try_from(symbols.len())
            .map_err(|_| unsupported("mana", "counted symbol quantity"))?;
        let amount = crate::operands::lower_for_each(
            for_each,
            &crate::ast::Amount::Fixed(Box::new(crate::ast::AmountFixed { value })),
            context,
        )?;
        return Ok((color, amount));
    }
    Err(unsupported("mana", "counted symbol bundle"))
}

/// CR 106.1a, 106.7: the complete production of the ability's one AddMana
/// step. A choice lists distinct fixed bundles. A "could produce" over anything but one land
/// controlled by "you" or "an opponent" rejects.
fn production(mana: &SourceMana, you: &PlayerSelector) -> Result<ManaProduction, Failure> {
    let unsupported_production = || unsupported("mana", "unsupported complete mana production");
    Ok(match mana {
        SourceMana::Fixed { symbols } if !symbols.is_empty() => ManaProduction::Fixed {
            symbols: symbols.clone(),
        },
        SourceMana::AnyColor { amount } if fixed_one(amount) => ManaProduction::AnyColor,
        SourceMana::ChosenColor { amount, choice }
            if *amount == serde_json::json!({"amount":"fixed","value":1})
                && *choice
                    == serde_json::json!({"choice":"chosen-value","category":"color","which":"chosen"}) =>
        {
            ManaProduction::SourceChosenColor
        }
        SourceMana::CommanderColor { amount, whose } if fixed_one(amount) && whose == you => {
            ManaProduction::CommanderColor
        }
        SourceMana::Choice { options } => {
            let mut bundles = vec![];
            for option in options {
                let SourceMana::Fixed { symbols } = option else {
                    return Err(unsupported_production());
                };
                let mut symbols = symbols.clone();
                symbols.sort();
                if symbols.is_empty() || symbols.len() > 5 || bundles.contains(&symbols) {
                    return Err(unsupported_production());
                }
                bundles.push(symbols);
            }
            if bundles.len() < 2 || bundles.len() > 128 {
                return Err(unsupported_production());
            }
            if bundles.iter().all(|bundle| bundle.len() == 1) {
                ManaProduction::Choose {
                    colors: bundles.into_iter().map(|bundle| bundle[0]).collect(),
                    amount: Quantity::from(1),
                }
            } else {
                ManaProduction::Bundles { options: bundles }
            }
        }
        SourceMana::Combination { amount, options } if options == "colors" => {
            let amount = fixed_amount(amount)?;
            match amount.to_u32() {
                Some(1) => ManaProduction::AnyColor,
                Some(n @ 2..=5) => {
                    let mut options = vec![];
                    let mut colors = ALL_COLORS;
                    colors.sort();
                    collect_bundles(&colors, n, &mut Vec::new(), &mut options);
                    ManaProduction::Bundles { options }
                }
                _ => return Err(unsupported_production()),
            }
        }
        SourceMana::AnyOneColor { amount } => ManaProduction::Choose {
            colors: ALL_COLORS.to_vec(),
            amount: fixed_amount(amount)?,
        },
        SourceMana::CouldProduce {
            amount,
            what,
            color_only,
        } if fixed_one(amount) => {
            let lands = |control: &str| {
                serde_json::json!({
                    "ref": "filter",
                    "filter": {"types": ["land"], "control": control},
                    "count": {"amount": "fixed", "value": 1}
                })
            };
            let controller = if *what == lands("you") {
                ManaLandController::You
            } else if *what == lands("opponent") {
                ManaLandController::Opponents
            } else {
                return Err(unsupported("mana", "could-produce source set"));
            };
            ManaProduction::LandsCouldProduce {
                controller,
                colors_only: *color_only,
            }
        }
        _ => return Err(unsupported_production()),
    })
}

/// CR 605.1a, 605.3b: an instruction after the AddMana step. The admitted
/// rows read only the ability's source and controller: "[this] deals N
/// damage to you" (CR 120.3a) and "you gain N life" (CR 119.3).
fn rider(step: &Step, is_self: &dyn Fn(&str) -> bool) -> Result<kir::Node, Failure> {
    let controller = || kir::Ref::controller(Context::Activated);
    let node = match step {
        Step::Evaluated(effect) => {
            if let ast::Effect::Damage(damage) = effect.as_ref()
                && let ast::EffectDamage {
                    optional: None,
                    decided_by: None,
                    starting_with: None,
                    for_each: None,
                    for_each_beyond: None,
                    for_each_vote: None,
                    duration: None,
                    source,
                    amount,
                    to,
                    unpreventable: None,
                    excess_to: None,
                    excess_condition: None,
                } = damage.as_ref()
                && match source.as_ref() {
                    ast::ObjectRef::This(this) => is_self(&this.noun),
                    ast::ObjectRef::SelfObject(_) => true,
                    _ => false,
                }
                && matches!(to.as_ref(), ast::DamageTarget::Player(to) if matches!(to.r#ref.as_ref(), ast::PlayerRef::You(_)))
            {
                return Ok(kir::Node::Do(Verb::Damage {
                    source: kir::Ref::Source,
                    to: kir::DamageTo::Ref { who: controller() },
                    amount: Amount::Fixed {
                        value: fixed_positive_amount(amount)?,
                    },
                }));
            }
            let ast::Effect::Variant99(life) = effect.as_ref() else {
                return Err(unsupported("mana", "unsupported mana ability rider"));
            };
            let ast::EffectVariant99 {
                optional: None,
                decided_by: None,
                starting_with: None,
                for_each: None,
                for_each_beyond: None,
                for_each_vote: None,
                duration: None,
                effect: ast::ReplacementEventVariant4Event::GainLife,
                who,
                amount,
            } = life.as_ref()
            else {
                return Err(unsupported("mana", "unsupported mana ability rider"));
            };
            if !matches!(who.as_ref(), ast::PlayerRef::You(_)) {
                return Err(unsupported("mana", "unsupported mana ability rider"));
            }
            kir::Node::Do(Verb::GainLife {
                player: PlayerExpr::you(Context::Activated),
                amount: kir::Amount::Fixed {
                    value: fixed_positive_amount(amount)?,
                },
            })
        }
        _ => return Err(unsupported("mana", "unsupported mana ability rider")),
    };
    kir::mana_rider(&node).map_err(|e| unsupported("mana", format!("mana ability rider: {e}")))?;
    Ok(node)
}

// CR 106.6: only exact, whole spending predicates and riders lower.
pub(crate) fn lower_spend_restriction(
    restriction: Option<&serde_json::Value>,
    riders: Option<&serde_json::Value>,
) -> Result<Option<ManaSpendRestriction>, Failure> {
    use ManaSpendRestriction as R;
    use serde_json::json;
    let only = |uses| json!({"restriction": "only", "uses": uses});
    let cast = |filter| json!({"use": "cast", "filter": filter});
    let activated = |source| json!({"use": "activate", "source": source});
    let restriction_matches = |uses| restriction == Some(&only(uses));
    let artifact = json!({"types": ["artifact"]});
    let artifact_spell = json!({"types": ["artifact"], "cls": "spell"});
    let legendary = json!({"supertypes": ["legendary"], "cls": "spell"});
    let instant_or_sorcery = json!({"allOf": [{"anyOf": [{"types":["instant"]}, {"types":["sorcery"]}]}], "cls":"spell"});
    let cant_counter = |uses| {
        json!([{
            "rider":"effect", "mana":"that", "spentOn":uses,
            "effects":[{"steps":[{"effect":"cant","what":{"ref":"that","filter":{"cls":"spell"}},"action":"be-countered"}]}]
        }])
    };
    // CR 607.2d, 106.6: bind the printed choice when the mana is produced,
    // then evaluate its permission and any rider at that exact payment.
    let chosen_filter = |cls| {
        json!({"types":["creature"],"cls":cls,
        "chosenType":{"choice":"chosen-type"}})
    };
    let chosen_spell = json!([cast(chosen_filter("spell"))]);
    let chosen_spell_or_ability = json!([
        cast(chosen_filter("spell")),
        activated(chosen_filter("source"))
    ]);
    let chosen = |ability_sources, on_spend| R::CreatureType {
        selection: mtg_contracts::mana_spend::CreatureTypeSpend::SourceChoice,
        ability_sources,
        on_spend,
    };
    if let Some(riders) = riders {
        let uses = json!([cast(legendary)]);
        return if restriction_matches(uses.clone()) && *riders == cant_counter(uses) {
            Ok(Some(R::LegendarySpellUncounterable))
        } else if restriction_matches(chosen_spell.clone()) && *riders == cant_counter(chosen_spell)
        {
            Ok(Some(chosen(
                false,
                Some(mtg_contracts::mana_spend::ManaSpendRider::UncounterableSpell),
            )))
        } else if restriction.is_none()
            && *riders == cant_counter(json!([cast(instant_or_sorcery)]))
        {
            Ok(Some(R::InstantOrSorceryUncounterable))
        } else {
            Err(unsupported("mana", "unsupported spend-time rider"))
        };
    }
    if restriction.is_none() {
        return Ok(None);
    }
    if restriction_matches(chosen_spell) {
        return Ok(Some(chosen(false, None)));
    }
    if restriction_matches(chosen_spell_or_ability) {
        return Ok(Some(chosen(true, None)));
    }
    if restriction_matches(json!([{"use":"activate"}])) {
        return Ok(Some(R::ActivatedAbility));
    }
    if restriction_matches(json!([cast(artifact_spell), activated(artifact.clone())])) {
        return Ok(Some(R::ArtifactSpellOrArtifactAbility));
    }
    if restriction_matches(json!([activated(artifact)])) {
        return Ok(Some(R::ArtifactAbility));
    }
    if restriction_matches(json!([cast(json!({"colorless":true,"cls":"spell"}))])) {
        return Ok(Some(R::ColorlessSpell));
    }
    if restriction_matches(json!([
        cast(json!({"colorless":true,"subtypes":["Eldrazi"],"cls":"spell"})),
        activated(json!({"colorless":true,"subtypes":["Eldrazi"]}))
    ])) {
        return Ok(Some(R::ColorlessEldraziSpellOrAbility));
    }
    for (name, kind) in [
        ("creature", CardKind::Creature),
        ("artifact", CardKind::Artifact),
        ("enchantment", CardKind::Enchantment),
        ("instant", CardKind::Instant),
        ("sorcery", CardKind::Sorcery),
    ] {
        if restriction_matches(json!([cast(json!({"types":[name],"cls":"spell"}))])) {
            return Ok(Some(R::SpellType { kind }));
        }
    }
    Err(unsupported("mana", "unsupported spend restriction"))
}

pub(crate) fn lower_mana_ability(
    costs: &[SourceCost],
    effects: &[Sentence],
    restriction: Option<&SourceActivationRestriction>,
    block: usize,
    kind: CardKind,
    is_self: &dyn Fn(&str) -> bool,
) -> Result<Option<ManaAbility>, Failure> {
    if !effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|s| matches!(s, Step::AddMana { .. }))
    {
        return Ok(None);
    }
    if effects
        .iter()
        .any(|s| !s.bindings.is_empty() || s.condition.is_some() || s.condition_kind.is_some())
    {
        return Err(unsupported(
            "bindings",
            "bindings outside spell instructions",
        ));
    }
    let mut steps = effects.iter().flat_map(|s| &s.steps).peekable();
    let you = PlayerSelector {
        player: "you".into(),
        selection_id: None,
        of: None,
    };
    let chose_color = matches!(steps.peek(), Some(Step::ChooseValue { who, category })
        if who == &you && category == "color");
    if chose_color {
        steps.next();
    }
    // CR 608.2c: the mana is added first, then the riders in printed order.
    let Some(Step::AddMana {
        who,
        mana,
        for_each,
        spend_restriction,
        riders: spend_riders,
    }) = steps.next()
    else {
        return Err(unsupported("mana", "extra mana steps"));
    };
    if *who != you {
        return Err(unsupported("mana", "unsupported mana recipient"));
    }
    let production = if chose_color {
        use serde_json::json;
        let chosen = json!({"choice":"chosen-value","category":"color","which":"chosen"});
        match mana {
            SourceMana::ChosenColor { amount, choice }
                if for_each.is_none()
                    && *choice == chosen
                    && *amount
                        == json!({"amount":"devotion","who":{"player":"you"},"colors":chosen})
                    && effects.len() == 2
                    && effects.iter().all(|sentence| sentence.steps.len() == 1) =>
            {
                ManaProduction::ChooseDevotion {}
            }
            _ => return Err(unsupported("mana", "chosen color devotion")),
        }
    } else if let Some(for_each) = for_each {
        let (color, amount) = counted_mana(mana, for_each, kir::Context::Activated)?;
        let (factor, count) = match amount {
            kir::Amount::Times { factor, of } => (factor, *of),
            count => (Quantity::from(1), count),
        };
        let kir::Amount::Count {
            set: kir::SetExpr::Matching { zone, filter },
        } = count
        else {
            return Err(unsupported("mana", "counted ability amount"));
        };
        ManaProduction::Counted {
            color,
            zone,
            factor,
            filter,
        }
    } else {
        production(mana, &you)?
    };
    let spend_restriction =
        lower_spend_restriction(spend_restriction.as_ref(), spend_riders.as_ref())?;
    let riders = steps
        .map(|step| {
            if matches!(step, Step::AddMana { .. }) {
                Err(unsupported("mana", "extra mana steps"))
            } else {
                rider(step, is_self)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (costs, hybrid_mana) = lower_permanent_costs(costs, kind, is_self)?;
    if !costs.tap && costs.remove_source_counters.is_some() {
        if costs.mana.is_some()
            || !hybrid_mana.is_empty()
            || costs.sacrifice
            || costs.pay_life.is_some()
            || restriction.is_none()
            || !riders.is_empty()
            || !matches!(&production, ManaProduction::Fixed { symbols } if !symbols.is_empty())
        {
            return Err(unsupported(
                "mana",
                "unsupported counter-removal mana shape",
            ));
        }
    } else if !costs.tap || restriction.is_some() {
        return Err(unsupported("mana", "unsupported mana cost/restriction"));
    }
    // A rider of a sacrificed source would read last-known information the
    // mana executor does not keep.
    if costs.sacrifice && !riders.is_empty() {
        return Err(unsupported("mana", "rider on a sacrificed mana source"));
    }
    Ok(Some(ManaAbility {
        restriction: restriction.map(|_| ManaActivationRestriction::OnceEachTurn),
        condition: None,
        costs,
        hybrid_mana,
        production,
        spend_restriction,
        riders,
        source_block: source_block(block)?,
    }))
}
