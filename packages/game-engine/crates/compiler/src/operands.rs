//! Operand lowerers: total functions from one typed AST operand (object,
//! player, filter, amount, target) to its IR operand, or an explicit rejection.
use super::*;

pub(crate) fn lower_damage_subject(
    what: &SourceDamageObject,
) -> Result<DamageTriggerSource, Failure> {
    match what {
        SourceDamageObject::This { noun } if noun == "creature" => {
            Ok(DamageTriggerSource::Source {})
        }
        SourceDamageObject::Filter { filter, count }
            if fixed_one(count)
                && filter.types.iter().any(|t| t == "creature")
                && filter.properties.is_none() =>
        {
            let controller = match filter.control.as_deref() {
                Some("you") => DamageSourceRelation::Controller,
                _ => {
                    return Err(unsupported(
                        "trigger.what.filter.control",
                        "unsupported damage source controller",
                    ));
                }
            };
            Ok(DamageTriggerSource::CreatureFilter {
                controller,
                types: lower_type_predicate(
                    &SourceTypeFilter {
                        colors: None,
                        cls: None,
                        types: Some(filter.types.clone()),
                        non_types: filter.non_types.clone(),
                        all_of: filter.all_of.clone(),
                        any_of: filter.any_of.clone(),
                    },
                    None,
                    true,
                    0,
                )?,
            })
        }
        _ => Err(unsupported(
            "trigger.what",
            "unsupported complete creature damage source/cardinality",
        )),
    }
}

pub(crate) fn lower_type_predicate(
    f: &SourceTypeFilter,
    class: Option<&str>,
    root: bool,
    depth: usize,
) -> Result<TypePredicate, Failure> {
    if depth > 8
        || (root && f.cls.as_deref() != class)
        || (f.cls.is_some() && f.cls.as_deref() != class)
    {
        return Err(unsupported("trigger.what", "unsupported spell class/depth"));
    }
    if let Some(colors) = &f.colors {
        if !root
            || class != Some("spell")
            || colors.len() != 1
            || f.types.is_some()
            || f.non_types.is_some()
            || f.all_of.is_some()
            || f.any_of.is_some()
        {
            return Err(unsupported(
                "trigger.what.colors",
                "expected one standalone spell color",
            ));
        }
        let color = match colors[0].as_str() {
            "white" => Color::W,
            "blue" => Color::U,
            "black" => Color::B,
            "red" => Color::R,
            "green" => Color::G,
            _ => return Err(unsupported("trigger.what.colors", "unknown spell color")),
        };
        return Ok(TypePredicate::SpellColor { color });
    }
    let kind = |s: &str| match s {
        "creature" => Ok(CardKind::Creature),
        "artifact" => Ok(CardKind::Artifact),
        "enchantment" => Ok(CardKind::Enchantment),
        "instant" => Ok(CardKind::Instant),
        "sorcery" => Ok(CardKind::Sorcery),
        "land" => Ok(CardKind::Land),
        _ => Err(unsupported("trigger.what.types", "unsupported spell type")),
    };
    let mut terms = vec![];
    for (values, negative) in [(&f.types, false), (&f.non_types, true)] {
        if let Some(values) = values {
            if values.is_empty() {
                return Err(unsupported("trigger.what", "empty type predicate"));
            }
            for t in values {
                let kind = kind(t)?;
                terms.push(if negative {
                    TypePredicate::NotType { kind }
                } else {
                    TypePredicate::Type { kind }
                });
            }
        }
    }
    for (children, disjunction) in [(&f.all_of, false), (&f.any_of, true)] {
        if let Some(children) = children {
            if children.is_empty() {
                return Err(unsupported("trigger.what", "empty boolean predicate"));
            }
            let children = children
                .iter()
                .map(|c| lower_type_predicate(c, class, false, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            terms.push(if disjunction {
                TypePredicate::AnyOf { terms: children }
            } else {
                TypePredicate::All { terms: children }
            });
        }
    }
    let p = match terms.len() {
        0 => TypePredicate::Any {},
        1 => terms.remove(0),
        _ => TypePredicate::All { terms },
    };
    p.validate().map_err(|e| unsupported("trigger.what", &e))?;
    Ok(p)
}

pub(crate) fn lower_attack_subject(what: &SourceAttacker) -> Result<AttackSubject, Failure> {
    match what {
        SourceAttacker::This { noun } if noun == "creature" => Ok(AttackSubject::Source {}),
        SourceAttacker::Filter { filter, count } if fixed_one(count) => {
            let controller = match filter.control.as_deref() {
                None => AttackerRelation::AnyPlayer,
                Some("you") => AttackerRelation::Controller,
                Some("opponent") => AttackerRelation::Opponent,
                _ => {
                    return Err(unsupported(
                        "trigger.what.filter.control",
                        "unsupported attacker controller",
                    ));
                }
            };
            Ok(AttackSubject::Filter {
                controller,
                types: lower_type_predicate(
                    &SourceTypeFilter {
                        colors: None,
                        cls: None,
                        types: filter.types.clone(),
                        non_types: filter.non_types.clone(),
                        all_of: filter.all_of.clone(),
                        any_of: filter.any_of.clone(),
                    },
                    None,
                    true,
                    0,
                )?,
            })
        }
        _ => Err(unsupported(
            "trigger.what",
            "unsupported complete attacker subject/cardinality",
        )),
    }
}

pub(crate) fn lower_temporary_creature_keywords(
    abilities: &[serde_json::Value],
) -> Result<Vec<KeywordAbility>, Failure> {
    let keywords = abilities
        .iter()
        .map(|ability| {
            let GainedAbility::Keyword { keyword } = serde_json::from_value(ability.clone())
                .map_err(|_| {
                    unsupported(
                        "effects.gain-abilities",
                        "unsupported gained keyword ability",
                    )
                })?;
            Ok(keyword.keyword)
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    if !supported_temporary_keywords(&keywords) {
        return Err(unsupported(
            "effects.gain-abilities",
            "expected nonempty unique reviewed temporary creature keywords",
        ));
    }
    Ok(keywords)
}

pub(crate) fn fixed_one(count: &Count) -> bool {
    count.amount == "fixed" && count.value == 1
}

pub(crate) fn source_block(block: usize) -> Result<u32, Failure> {
    u32::try_from(block).map_err(|_| invalid("abilities", "structural block limit"))
}

pub(crate) fn bind_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    id: &str,
    domain: TargetDomain,
) -> Result<u32, Failure> {
    if id.is_empty() || program.targets.iter().any(|target| target.domain != domain) {
        return Err(unsupported(
            "targets",
            "empty selection or mixed creature/player target domains",
        ));
    }
    if let Some(slot) = slots.get(id) {
        return Ok(*slot);
    }
    let slot = u32::try_from(program.targets.len())
        .map_err(|_| invalid("targets", "structural slot limit"))?;
    program.targets.push(TargetSpec::new(slot, id, domain));
    slots.insert(id.into(), slot);
    Ok(slot)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerBinding {
    Unbound,
    /// "That player" is the trigger's event player in this role.
    EventPlayer(kir::EventPlayerRole),
    DamageRecipient,
    DeathController,
}

impl PlayerBinding {
    pub(crate) fn event_role(self) -> Option<kir::EventPlayerRole> {
        match self {
            Self::EventPlayer(role) => Some(role),
            _ => None,
        }
    }
}

// Pinned CR 115, 608.2b/h and 202.3a: bind the complete finite filter and
// target reference; no unparsed attribute or controller clause is discarded.
/// Closed source-backed sets: never silently discard filter fields (CR 608.2h).
pub(crate) fn lower_object_set(v: &serde_json::Value) -> Result<ObjectSet, Failure> {
    use serde_json::json;
    if v["ref"] != "each" && v["ref"] != "all" {
        return Err(unsupported("set", "expected all/each set"));
    }
    if v.as_object().is_none_or(|o| o.len() != 2) {
        return Err(unsupported("set", "extra set fields"));
    }
    let f = &v["filter"];
    let set = if *f == json!({"types":["creature"]}) {
        ObjectSet::Creatures
    } else if *f == json!({"nonTypes":["land"],"cls":"permanent"}) {
        ObjectSet::NonlandPermanents
    } else if *f == json!({"types":["enchantment"]}) {
        ObjectSet::Enchantments
    } else if *f == json!({"types":["creature"],"status":["attacking"]}) {
        ObjectSet::AttackingCreatures
    } else if *f == json!({"cls":"permanent","control":"you"}) {
        // CR 109.2/109.5, 608.2h, 611.2c: every controlled permanent,
        // captured by the ordinary keyword program; never narrow to creatures.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        ObjectSet::ControllerPermanents
    } else if *f == json!({"types":["creature"],"control":"you"}) {
        ObjectSet::ControllerCreatures
    } else if *f == json!({"types":["creature"],"control":"opponent"}) {
        ObjectSet::OpponentCreatures
    } else if *f == json!({"types":["creature"],"withAbility":["flying"]}) {
        ObjectSet::FlyingCreatures
    } else if *f == json!({"types":["creature"],"withoutAbility":["flying"]}) {
        ObjectSet::NonflyingCreatures
    } else {
        return Err(unsupported("set", "unsupported complete filter"));
    };
    Ok(set)
}

/// CR 608.2h: an "all"/"each" operand of a destroy, exile or return step is
/// the set of battlefield permanents that satisfy its whole filter when the
/// instruction is performed. Every printed filter term becomes one predicate
/// atom, in this fixed order; a term with no atom rejects the step.
pub(crate) fn lower_board_set(v: &serde_json::Value, context: Context) -> Result<SetExpr, Failure> {
    if v["ref"] != "each" && v["ref"] != "all" {
        return Err(unsupported("set", "expected all/each set"));
    }
    if v.as_object().is_none_or(|o| o.len() != 2) {
        return Err(unsupported("set", "extra set fields"));
    }
    Ok(SetExpr::battlefield(lower_board_filter(
        &v["filter"],
        context,
    )?))
}

pub(crate) fn lower_board_filter(
    filter: &serde_json::Value,
    context: Context,
) -> Result<Pred, Failure> {
    let unsupported_term = || unsupported("set", "unsupported complete filter");
    let object = filter.as_object().ok_or_else(unsupported_term)?;
    const TERMS: [&str; 15] = [
        "cls",
        "types",
        "allOf",
        "nonTypes",
        "supertypes",
        "nonSupertypes",
        "subtypes",
        "nonSubtypes",
        "colors",
        "nonColors",
        "control",
        "status",
        "nonToken",
        "withAbility",
        "withoutAbility",
    ];
    if object
        .keys()
        .any(|key| !TERMS.contains(&key.as_str()) && key != "properties")
    {
        return Err(unsupported_term());
    }
    let list = |key: &str| -> Result<Vec<&serde_json::Value>, Failure> {
        match object.get(key) {
            None => Ok(vec![]),
            Some(value) => value
                .as_array()
                .filter(|values| !values.is_empty())
                .map(|values| values.iter().collect())
                .ok_or_else(unsupported_term),
        }
    };
    let kind = |value: &serde_json::Value| {
        serde_json::from_value::<CardKind>(value.clone()).map_err(|_| unsupported_term())
    };
    let name = |value: &serde_json::Value| {
        value
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(unsupported_term)
    };
    let color = |value: &serde_json::Value| match value.as_str() {
        Some("white") => Ok(Color::W),
        Some("blue") => Ok(Color::U),
        Some("black") => Ok(Color::B),
        Some("red") => Ok(Color::R),
        Some("green") => Ok(Color::G),
        _ => Err(unsupported_term()),
    };
    let mut atoms = vec![];
    // CR 110.1: "permanent" names the zone, which the set already fixes.
    match object.get("cls").map(|cls| cls.as_str()) {
        None | Some(Some("permanent")) => {}
        Some(Some("token")) => atoms.push(Atom::Token),
        _ => return Err(unsupported_term()),
    }
    // CR 205.2: a list of card types requires every one of them.
    for value in list("types")? {
        atoms.push(Atom::Type { kind: kind(value)? });
    }
    // The parser spells "artifact or enchantment" as one group of alternatives.
    if let Some(groups) = object.get("allOf") {
        let [group] = groups.as_array().ok_or_else(unsupported_term)?.as_slice() else {
            return Err(unsupported_term());
        };
        let alternatives = group
            .as_object()
            .filter(|o| o.len() == 1)
            .and_then(|o| o.get("anyOf"))
            .and_then(|a| a.as_array())
            .filter(|a| a.len() >= 2)
            .ok_or_else(unsupported_term)?;
        let mut kinds = vec![];
        for alternative in alternatives {
            let types = alternative
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("types"))
                .and_then(|t| t.as_array())
                .ok_or_else(unsupported_term)?;
            let [only] = types.as_slice() else {
                return Err(unsupported_term());
            };
            kinds.push(kind(only)?);
        }
        atoms.push(Atom::AnyType { kinds });
    }
    for value in list("nonTypes")? {
        atoms.push(Atom::NotType { kind: kind(value)? });
    }
    for (key, legendary, basic) in [
        ("supertypes", Atom::Legendary, Atom::Basic),
        ("nonSupertypes", Atom::NotLegendary, Atom::NotBasic),
    ] {
        for value in list(key)? {
            atoms.push(match value.as_str() {
                Some("legendary") => legendary.clone(),
                Some("basic") => basic.clone(),
                _ => return Err(unsupported_term()),
            });
        }
    }
    for value in list("subtypes")? {
        atoms.push(Atom::Subtype {
            subtype: name(value)?,
        });
    }
    for value in list("nonSubtypes")? {
        atoms.push(Atom::NotSubtype {
            subtype: name(value)?,
        });
    }
    for value in list("colors")? {
        atoms.push(Atom::Color {
            color: color(value)?,
        });
    }
    for value in list("nonColors")? {
        atoms.push(Atom::NotColor {
            color: color(value)?,
        });
    }
    match object.get("control").map(|c| c.as_str()) {
        None => {}
        // CR 109.5: "you control" is the controller of this stack object.
        Some(Some("you")) => atoms.push(Atom::ControlledBy {
            player: PlayerExpr::you(context),
        }),
        // CR 102.3: "your opponents control".
        Some(Some("opponent")) => atoms.push(Atom::ControlledBy {
            player: PlayerExpr::EachOpponent,
        }),
        _ => return Err(unsupported_term()),
    }
    for value in list("status")? {
        atoms.push(match value.as_str() {
            Some("tapped") => Atom::Tapped,
            Some("untapped") => Atom::Untapped,
            Some("attacking") => Atom::Attacking,
            _ => return Err(unsupported_term()),
        });
    }
    match object.get("nonToken") {
        None => {}
        Some(serde_json::Value::Bool(true)) => atoms.push(Atom::NotToken),
        _ => return Err(unsupported_term()),
    }
    for (key, has) in [("withAbility", true), ("withoutAbility", false)] {
        for value in list(key)? {
            // CR 702.9: flying is the one keyword the set evaluator reviews.
            if value != "flying" {
                return Err(unsupported_term());
            }
            let keyword = KeywordAbility::Flying;
            atoms.push(if has {
                Atom::Keyword { keyword }
            } else {
                Atom::NotKeyword { keyword }
            });
        }
    }
    // CR 208.1, 202.3: a fixed comparison of power, toughness or mana value.
    for value in list("properties")? {
        let property = value
            .as_object()
            .filter(|o| o.len() == 2)
            .ok_or_else(unsupported_term)?;
        let comparison = property
            .get("comparison")
            .and_then(|c| c.as_object())
            .filter(|c| c.len() == 2)
            .ok_or_else(unsupported_term)?;
        let bound = comparison
            .get("value")
            .filter(|v| v["amount"] == "fixed" && v.as_object().is_some_and(|o| o.len() == 2))
            .and_then(|v| v["value"].as_i64())
            .ok_or_else(unsupported_term)?;
        // Integers: "less than N" is "at most N-1" (CR 107.1).
        let (cmp, bound) = match comparison.get("op").and_then(|op| op.as_str()) {
            Some("ge") => (Cmp::AtLeast, bound),
            Some("gt") => (
                Cmp::AtLeast,
                bound.checked_add(1).ok_or_else(unsupported_term)?,
            ),
            Some("le") => (Cmp::AtMost, bound),
            Some("lt") => (
                Cmp::AtMost,
                bound.checked_sub(1).ok_or_else(unsupported_term)?,
            ),
            Some("eq") => (Cmp::Exactly, bound),
            _ => return Err(unsupported_term()),
        };
        let value = Amount::fixed(bound);
        atoms.push(match property.get("property").and_then(|p| p.as_str()) {
            Some("power") => Atom::Power { cmp, value },
            Some("toughness") => Atom::Toughness { cmp, value },
            Some("mana-value") => Atom::ManaValue { cmp, value },
            _ => return Err(unsupported_term()),
        });
    }
    Ok(Pred(atoms))
}

pub(crate) fn lower_creature_token(token: &SourceToken) -> Result<CreatureToken, Failure> {
    if !token.supertypes.is_empty()
        || token.power.amount != "fixed"
        || token.toughness.amount != "fixed"
    {
        return Err(unsupported(
            "effects.token",
            "unsupported token characteristic shape",
        ));
    }
    let additional_types = match token.types.as_slice() {
        [kind] if kind == "creature" => vec![],
        [a, b] if a == "creature" || b == "creature" => {
            let secondary = if a == "creature" { b } else { a };
            match secondary.as_str() {
                "artifact" => vec![CardKind::Artifact],
                "enchantment" => vec![CardKind::Enchantment],
                _ => {
                    return Err(unsupported(
                        "effects.token.types",
                        "unsupported secondary type",
                    ));
                }
            }
        }
        _ => {
            return Err(unsupported(
                "effects.token.types",
                "unsupported token types",
            ));
        }
    };
    let colors = token
        .colors
        .iter()
        .map(|c| match c.as_str() {
            "white" => Ok(Color::W),
            "blue" => Ok(Color::U),
            "black" => Ok(Color::B),
            "red" => Ok(Color::R),
            "green" => Ok(Color::G),
            _ => Err(unsupported("effects.token.colors", "unsupported color")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let keywords = token
        .keywords
        .iter()
        .map(|k| {
            let obj = k
                .as_object()
                .ok_or_else(|| unsupported("effects.token.keywords", "invalid keyword"))?;
            if obj.len() != 1 {
                return Err(unsupported(
                    "effects.token.keywords",
                    "parameterized keyword",
                ));
            }
            serde_json::from_value::<KeywordAbility>(k["keyword"].clone())
                .map_err(|_| unsupported("effects.token.keywords", "unsupported keyword"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let token = CreatureToken {
        additional_types,
        colors,
        subtypes: token.subtypes.clone(),
        power: i64::from(token.power.value).into(),
        toughness: i64::from(token.toughness.value).into(),
        keywords,
    };
    token
        .validate()
        .map_err(|e| unsupported("effects.token", e))?;

    Ok(token)
}

pub(crate) fn lower_permanent_filter(v: &serde_json::Value) -> Result<PermanentFilter, Failure> {
    let kind = |v: &serde_json::Value| -> Result<CardKind, Failure> {
        serde_json::from_value(v.clone())
            .map_err(|_| unsupported("filter", "unsupported card type"))
    };
    let mut types = vec![];
    let obj = v
        .as_object()
        .ok_or_else(|| unsupported("filter", "not an object"))?;
    if obj.keys().any(|k| {
        ![
            "cls",
            "types",
            "allOf",
            "nonTypes",
            "nonColors",
            "control",
            "status",
        ]
        .contains(&k.as_str())
    }) {
        return Err(unsupported("filter", "unsupported filter term"));
    }
    if let Some(a) = v.get("types") {
        let a = a.as_array().ok_or_else(|| unsupported("filter", "types"))?;
        // ObjectFilter.types requires every type (oracle-parser/src/ast.ts),
        // whereas PermanentFilter.types contains alternatives. Do not widen
        // target criteria: pinned CR 205.2b, 601.2c and 608.2b.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if a.len() > 1 {
            return Err(unsupported("filter.types", "unsupported type conjunction"));
        }
        for t in a {
            types.push(kind(t)?);
        }
    }
    if let Some(a) = v.get("allOf") {
        if !types.is_empty() {
            return Err(unsupported("filter", "mixed type forms"));
        }
        let a = a.as_array().ok_or_else(|| unsupported("filter", "allOf"))?;
        if a.len() != 1 || a[0].as_object().is_none_or(|o| o.len() != 1) {
            return Err(unsupported("filter", "unsupported conjunction"));
        }
        let alternatives = a[0]["anyOf"]
            .as_array()
            .ok_or_else(|| unsupported("filter", "anyOf"))?;
        if alternatives.len() < 2 {
            return Err(unsupported("filter", "empty alternatives"));
        }
        for a in alternatives {
            if a.as_object().is_none_or(|o| o.len() != 1)
                || a["types"].as_array().is_none_or(|a| a.len() != 1)
            {
                return Err(unsupported("filter", "unsupported alternative"));
            }
            types.push(kind(&a["types"][0])?);
        }
    }
    if let Some(cls) = v.get("cls")
        && (cls != "permanent" || !types.is_empty())
    {
        return Err(unsupported("filter", "unsupported class"));
    }
    if types.is_empty() && v.get("cls").is_none() {
        return Err(unsupported("filter", "missing type/class"));
    }
    let non_types = v
        .get("nonTypes")
        .map(|a| {
            a.as_array()
                .ok_or_else(|| unsupported("filter", "nonTypes"))
                .and_then(|a| a.iter().map(kind).collect())
        })
        .transpose()?
        .unwrap_or_default();
    let non_colors = v
        .get("nonColors")
        .map(|a| {
            a.as_array()
                .ok_or_else(|| unsupported("filter", "nonColors"))
                .and_then(|a| {
                    a.iter()
                        .map(|c| match c.as_str() {
                            Some("black") => Ok(Color::B),
                            _ => Err(unsupported("filter", "unsupported noncolor")),
                        })
                        .collect()
                })
        })
        .transpose()?
        .unwrap_or_default();
    let opponent = match v.get("control") {
        None => false,
        Some(c) if c == "opponent" => true,
        _ => return Err(unsupported("filter", "unsupported controller")),
    };
    // CR 110.5/601.2c/608.2b: this predicate tests the permanent's
    // current tapped status. Every source status term must be represented.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let tapped = match v.get("status") {
        None => false,
        Some(status) if *status == serde_json::json!(["tapped"]) => true,
        _ => {
            return Err(unsupported(
                "filter.status",
                "unsupported complete permanent status",
            ));
        }
    };
    Ok(PermanentFilter {
        power: None,
        types,
        non_types,
        non_colors,
        opponent,
        tapped,
    })
}

pub(crate) fn bind_permanent_target(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    v: &serde_json::Value,
) -> Result<u32, Failure> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct T {
        #[serde(rename = "ref")]
        reference: String,
        filter: serde_json::Value,
        count: Count,
        selection_id: String,
    }
    let t: T = serde_json::from_value(v.clone())
        .map_err(|_| unsupported("target", "unsupported complete permanent target"))?;
    if t.reference != "target" || !fixed_one(&t.count) {
        return Err(unsupported("target", "requires one target"));
    }
    let f = lower_permanent_filter(&t.filter)?;
    let slot = bind_target(
        p,
        slots,
        &t.selection_id,
        TargetDomain::BattlefieldPermanent,
    )?;
    if p.targets[slot as usize]
        .filter
        .as_ref()
        .is_some_and(|old| old != &f)
    {
        return Err(unsupported("filter", "conflicting target filter"));
    }
    p.targets[slot as usize].filter = Some(f);
    Ok(slot)
}

pub(crate) fn target_reference(p: &SpellProgram, v: &serde_json::Value) -> Result<u32, Failure> {
    if p.targets.len() != 1
        || !matches!(
            p.targets[0].domain,
            TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
        )
    {
        return Err(unsupported("reference", "requires bound removal target"));
    }
    let valid = *v == serde_json::json!({"ref":"it"})
        || *v == serde_json::json!({"ref":"that","filter":{"cls":"permanent"}})
        || (*v == serde_json::json!({"ref":"that","filter":{"types":["artifact"]}})
            && p.targets[0]
                .filter
                .as_ref()
                .is_some_and(|f| f.types == [CardKind::Artifact]));
    if !valid {
        return Err(unsupported(
            "reference",
            "unsupported complete target reference",
        ));
    }
    Ok(0)
}

pub(crate) fn removal_recipient(
    p: &SpellProgram,
    v: &serde_json::Value,
) -> Result<EffectRecipient, Failure> {
    if *v == serde_json::json!({"player":"you"}) {
        return Ok(EffectRecipient::Controller);
    }
    if v.as_object().is_none_or(|o| o.len() != 2) || v["player"] != "controller" {
        return Err(unsupported("recipient", "unsupported target controller"));
    }
    Ok(EffectRecipient::TargetController {
        target_slot: target_reference(p, &v["of"])?,
    })
}

pub(crate) fn removal_amount(
    p: &SpellProgram,
    v: &serde_json::Value,
) -> Result<TargetAmount, Failure> {
    if v["amount"] == "attribute" && v.as_object().is_some_and(|o| o.len() == 3) {
        let attribute = match v["attribute"].as_str() {
            Some("power") => TargetAttribute::Power,
            Some("mana-value") => TargetAttribute::ManaValue,
            _ => return Err(unsupported("amount", "unsupported attribute")),
        };
        return Ok(TargetAmount::Attribute {
            target_slot: target_reference(p, &v["of"])?,
            attribute,
        });
    }
    Ok(TargetAmount::Fixed {
        amount: fixed_positive(
            &serde_json::from_value(v.clone())
                .map_err(|_| unsupported("amount", "unsupported amount"))?,
        )?,
    })
}

pub(crate) fn signed_fixed(value: &SignedAmount) -> Result<Quantity, Failure> {
    if value.amount.amount != "fixed" || !matches!(value.sign, -1 | 1) {
        return Err(unsupported(
            "effects.pump",
            "unsupported signed fixed amount",
        ));
    }
    Ok(Quantity::from(
        i64::from(value.amount.value) * i64::from(value.sign),
    ))
}

pub(crate) fn fixed_positive_value(value: &serde_json::Value) -> Result<Quantity, Failure> {
    fixed_positive(
        &serde_json::from_value(value.clone())
            .map_err(|_| unsupported("amount", "unsupported damage amount"))?,
    )
}

pub(crate) fn fixed_positive(count: &Count) -> Result<Quantity, Failure> {
    if count.amount != "fixed" || count.value == 0 {
        return Err(unsupported(
            "amount",
            "only positive fixed quantities are supported",
        ));
    }
    Ok(Quantity::from(i64::from(count.value)))
}

/// CR 109.5, 119.3: who a life change names. "You" is the controller of this
/// kind of stack object; "that player" is the trigger's event player.
pub(crate) fn life_player(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    who: &PlayerSelector,
    context: Context,
    event_player: Option<kir::EventPlayerRole>,
) -> Result<PlayerExpr, Failure> {
    if who.of.is_some() {
        return Err(unsupported("who", "unexpected object reference"));
    }
    match (
        who.player.as_str(),
        who.selection_id.as_deref(),
        event_player,
    ) {
        ("you", None, _) => Ok(PlayerExpr::you(context)),
        ("that-player", None, Some(role)) => Ok(PlayerExpr::event(role)),
        ("each-opponent", None, _) => Ok(PlayerExpr::EachOpponent),
        ("target-player", Some(id), _) => Ok(PlayerExpr::target(bind_target(
            program,
            slots,
            id,
            TargetDomain::AnyPlayer,
        )?)),
        _ => Err(unsupported(
            "who",
            "unsupported life recipient or unbound target",
        )),
    }
}

// CR 509.1g/601.2c/608.2b: consume the entire blocking-creature target.
// The blocking exception stays pump-only; keyword grants use the ordinary/controlled binder.
pub(crate) fn bind_pump_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &serde_json::Value,
) -> Result<u32, Failure> {
    let id = what["selectionId"].as_str().unwrap_or("");
    if *what
        == serde_json::json!({"ref":"target","filter":{"status":["blocking"],"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":id})
    {
        // bind_target rejects mixed domains before looking up a reused ID, so
        // a second use cannot discard or overwrite the blocking restriction.
        return bind_target(
            program,
            slots,
            id,
            TargetDomain::BlockingBattlefieldCreature,
        );
    }
    bind_temporary_creature_target(program, slots, what)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlledPermanentTarget {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) filter: ControlledPermanentFilter,
    pub(crate) count: Count,
    #[serde(rename = "selectionId")]
    pub(crate) selection_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControlledPermanentFilter {
    pub(crate) cls: String,
    pub(crate) control: String,
}

pub(crate) fn bind_temporary_permanent_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &serde_json::Value,
) -> Result<u32, Failure> {
    let target: ControlledPermanentTarget = serde_json::from_value(what.clone())
        .map_err(|_| unsupported("targets", "requires complete controlled permanent target"))?;
    if target.reference != "target"
        || target.filter.cls != "permanent"
        || target.filter.control != "you"
        || !fixed_one(&target.count)
    {
        return Err(unsupported(
            "targets",
            "requires exact controlled permanent target",
        ));
    }
    let filter = Some(PermanentFilter::any());
    if let Some(slot) = slots.get(&target.selection_id)
        && program
            .targets
            .get(*slot as usize)
            .is_none_or(|t| t.filter != filter)
    {
        return Err(unsupported(
            "targets",
            "inconsistent reused permanent selection",
        ));
    }
    let slot = bind_target(
        program,
        slots,
        &target.selection_id,
        TargetDomain::ControlledBattlefieldPermanent,
    )?;
    program.targets[slot as usize].filter = filter;
    Ok(slot)
}

// CR 109.5/601.2c/608.2b: “you” is the invocation controller and remains a
// target restriction, not an owner test. Consume the exact controlled shape
// before the ordinary strict decoder; null/other actors and extra filters fail.
// Only pump and temporary keyword grants use this binder. Contracts require a
// complete keyword program for controlled pumps; other creature operators keep
// bind_creature_target's unqualified scope.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn bind_temporary_creature_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &serde_json::Value,
) -> Result<u32, Failure> {
    let id = what["selectionId"].as_str().unwrap_or("");
    if *what
        == serde_json::json!({"ref":"target","filter":{"types":["creature"],"control":"you"},
        "count":{"amount":"fixed","value":1},"selectionId":id})
    {
        // bind_target checks domain consistency before reusing any selection
        // slot, so a second operation cannot drop the controller restriction.
        return bind_target(
            program,
            slots,
            id,
            TargetDomain::ControlledBattlefieldCreature,
        );
    }
    let target = serde_json::from_value(what.clone())
        .map_err(|_| unsupported("targets", "unsupported complete temporary creature target"))?;
    bind_creature_target(program, slots, &target)
}

pub(crate) fn bind_creature_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    target: &Target,
) -> Result<u32, Failure> {
    if target.reference != "target"
        || target.filter.types != ["creature"]
        || !fixed_one(&target.count)
    {
        return Err(unsupported(
            "targets",
            "unsupported creature target scope/cardinality",
        ));
    }
    bind_target(
        program,
        slots,
        &target.selection_id,
        TargetDomain::BattlefieldCreature,
    )
}

pub(crate) fn is_source_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::This {noun} if noun=="creature")
}

pub(crate) fn is_other_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::Filter {filter,count} if filter.properties.is_none() && filter.other && filter.types==["creature"] && filter.control.is_none() && !filter.non_token && filter.non_subtypes.is_empty() && fixed_one(count))
}
