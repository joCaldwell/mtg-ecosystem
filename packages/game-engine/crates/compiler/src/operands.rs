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

// ---------------------------------------------------------------------------
// Typed operands. Each lowerer reads one operand kind as its complete
// generated AST type (`crate::ast`) and either returns its IR operand or
// rejects under `operand:<kind>:<shape>`: the AST field or variant it does
// not represent. No printed field is skipped and no JSON literal is compared.
// Pinned CR 115, 601.2c, 608.2b-h, 202.3a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// ---------------------------------------------------------------------------

/// Decode one printed operand as its generated AST type. The block already
/// decoded as the complete AST, so a failure here is an archive defect.
pub(crate) fn typed<T: serde::de::DeserializeOwned>(
    value: &impl serde::Serialize,
    kind: &str,
) -> Result<T, Failure> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|e| invalid("operand", format!("operand:{kind}: {e}")))
}

/// The named rejection of one operand shape.
pub(crate) fn operand(kind: &str, shape: impl AsRef<str>) -> Failure {
    unsupported("operand", format!("operand:{kind}:{}", shape.as_ref()))
}

/// The snake-case name of a generated enum variant, for a rejection code.
pub(crate) fn variant<T: std::fmt::Debug>(value: &T) -> String {
    let debug = format!("{value:?}");
    let name: String = debug.chars().take_while(|c| c.is_alphanumeric()).collect();
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// CR 608.2h: an "all"/"each" operand is the set of battlefield permanents
/// that satisfy its whole filter when the instruction is performed.
pub(crate) fn lower_board_set(what: &ast::ObjectRef, context: Context) -> Result<SetExpr, Failure> {
    match what {
        ast::ObjectRef::All(set) => Ok(SetExpr::battlefield(lower_filter(&set.filter, context)?)),
        ast::ObjectRef::Each(set) => Ok(SetExpr::battlefield(lower_filter(&set.filter, context)?)),
        other => Err(operand("set", format!("ref_{}", variant(other)))),
    }
}

/// The board sets an end-of-turn pump or a set damage step was reviewed
/// for (CR 613.4c, 120.2b). Kernel cells audit any creature set; these
/// lowerers keep the reviewed closed sets until their wave widens them.
pub(crate) fn lower_reviewed_set(
    what: &ast::ObjectRef,
    context: Context,
) -> Result<SetExpr, Failure> {
    const REVIEWED: [ObjectSet; 9] = [
        ObjectSet::Creatures,
        ObjectSet::NonlandPermanents,
        ObjectSet::Enchantments,
        ObjectSet::AttackingCreatures,
        ObjectSet::ControllerPermanents,
        ObjectSet::ControllerCreatures,
        ObjectSet::OpponentCreatures,
        ObjectSet::FlyingCreatures,
        ObjectSet::NonflyingCreatures,
    ];
    let set = lower_board_set(what, context)?;
    if !REVIEWED
        .iter()
        .any(|reviewed| reviewed.kernel(context) == set)
    {
        return Err(operand("set", "unreviewed_set"));
    }
    Ok(set)
}

/// CR 205.2a: a printed card type, by its AST spelling.
fn card_kind(kind: &str, name: &str) -> Result<CardKind, Failure> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned()))
        .map_err(|_| operand(name, format!("type:{kind}")))
}

/// CR 105.1: the five colors.
fn color(color: &ast::Color) -> Color {
    match color {
        ast::Color::White => Color::W,
        ast::Color::Blue => Color::U,
        ast::Color::Black => Color::B,
        ast::Color::Red => Color::R,
        ast::Color::Green => Color::G,
    }
}

/// A printed list term: present lists are never empty.
fn listed<'a, T>(list: &'a Option<Vec<T>>, field: &str) -> Result<&'a [T], Failure> {
    match list {
        None => Ok(&[]),
        Some(items) if items.is_empty() => Err(operand("filter", format!("{field}:empty"))),
        Some(items) => Ok(items),
    }
}

/// "artifact or enchantment": one `allOf` group of single-type alternatives.
fn type_alternatives(all_of: &[ast::ObjectFilter], kind: &str) -> Result<Vec<CardKind>, Failure> {
    let [group] = all_of else {
        return Err(operand(kind, "all_of:conjunction"));
    };
    let alternatives = match (only_field(group), group.any_of.as_deref()) {
        (Some("any_of"), Some(alternatives)) => alternatives,
        _ => return Err(operand(kind, "all_of:group")),
    };
    if alternatives.len() < 2 {
        return Err(operand(kind, "any_of:alternatives"));
    }
    alternatives
        .iter()
        .map(
            |alternative| match (only_field(alternative), alternative.types.as_deref()) {
                (Some("types"), Some([only])) => card_kind(only, kind),
                _ => Err(operand(kind, "any_of:alternative")),
            },
        )
        .collect()
}

/// CR 608.2h: every printed term of a board filter becomes one predicate
/// atom, in this fixed order; a term with no atom rejects the operand.
pub(crate) fn lower_filter(filter: &ast::ObjectFilter, context: Context) -> Result<Pred, Failure> {
    if let Some(field) = present_fields(filter)
        .into_iter()
        .find(|field| !BOARD_FILTER_TERMS.contains(field))
    {
        return Err(operand("filter", field));
    }
    let mut atoms = vec![];
    // CR 110.1: "permanent" names the zone, which the set already fixes.
    match &filter.cls {
        None | Some(ast::ObjectFilterCls::Permanent) => {}
        Some(ast::ObjectFilterCls::Token) => atoms.push(Atom::Token),
        Some(other) => return Err(operand("filter", format!("cls:{}", variant(other)))),
    }
    // CR 205.2: a list of card types requires every one of them.
    for kind in listed(&filter.types, "types")? {
        atoms.push(Atom::Type {
            kind: card_kind(kind, "filter")?,
        });
    }
    if let Some(all_of) = &filter.all_of {
        atoms.push(Atom::AnyType {
            kinds: type_alternatives(all_of, "filter")?,
        });
    }
    for kind in listed(&filter.non_types, "non_types")? {
        atoms.push(Atom::NotType {
            kind: card_kind(kind, "filter")?,
        });
    }
    for (list, field, legendary, basic) in [
        (
            &filter.supertypes,
            "supertypes",
            Atom::Legendary,
            Atom::Basic,
        ),
        (
            &filter.non_supertypes,
            "non_supertypes",
            Atom::NotLegendary,
            Atom::NotBasic,
        ),
    ] {
        for supertype in listed(list, field)? {
            atoms.push(match supertype.as_str() {
                "legendary" => legendary.clone(),
                "basic" => basic.clone(),
                other => return Err(operand("filter", format!("{field}:{other}"))),
            });
        }
    }
    for subtype in listed(&filter.subtypes, "subtypes")? {
        if subtype.is_empty() {
            return Err(operand("filter", "subtypes:empty"));
        }
        atoms.push(Atom::Subtype {
            subtype: subtype.clone(),
        });
    }
    for subtype in listed(&filter.non_subtypes, "non_subtypes")? {
        if subtype.is_empty() {
            return Err(operand("filter", "non_subtypes:empty"));
        }
        atoms.push(Atom::NotSubtype {
            subtype: subtype.clone(),
        });
    }
    for c in listed(&filter.colors, "colors")? {
        atoms.push(Atom::Color { color: color(c) });
    }
    for c in listed(&filter.non_colors, "non_colors")? {
        atoms.push(Atom::NotColor { color: color(c) });
    }
    match &filter.control {
        None => {}
        // CR 109.5: "you control" is the controller of this stack object.
        Some(ast::ObjectFilterControl::You) => atoms.push(Atom::ControlledBy {
            player: PlayerExpr::you(context),
        }),
        // CR 102.3: "your opponents control".
        Some(ast::ObjectFilterControl::Opponent) => atoms.push(Atom::ControlledBy {
            player: PlayerExpr::EachOpponent,
        }),
        Some(other) => return Err(operand("filter", format!("control:{}", variant(other)))),
    }
    for status in listed(&filter.status, "status")? {
        atoms.push(match status.as_str() {
            "tapped" => Atom::Tapped,
            "untapped" => Atom::Untapped,
            "attacking" => Atom::Attacking,
            other => return Err(operand("filter", format!("status:{other}"))),
        });
    }
    match filter.non_token {
        None => {}
        Some(true) => atoms.push(Atom::NotToken),
        Some(false) => return Err(operand("filter", "non_token:false")),
    }
    for (list, field, has) in [
        (&filter.with_ability, "with_ability", true),
        (&filter.without_ability, "without_ability", false),
    ] {
        for ability in listed(list, field)? {
            // CR 702.9: flying is the one keyword the set evaluator reviews.
            if ability != "flying" {
                return Err(operand("filter", format!("{field}:{ability}")));
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
    for property in listed(&filter.properties, "properties")? {
        let ast::Amount::Fixed(bound) = &*property.comparison.value else {
            return Err(operand(
                "filter",
                format!("properties:{}", variant(&*property.comparison.value)),
            ));
        };
        let bound = bound.value;
        let overflow = || operand("filter", "properties:bound");
        // Integers: "less than N" is "at most N-1" (CR 107.1).
        let (cmp, bound) = match property.comparison.op {
            ast::ComparisonOp::Ge => (Cmp::AtLeast, bound),
            ast::ComparisonOp::Gt => (Cmp::AtLeast, bound.checked_add(1).ok_or_else(overflow)?),
            ast::ComparisonOp::Le => (Cmp::AtMost, bound),
            ast::ComparisonOp::Lt => (Cmp::AtMost, bound.checked_sub(1).ok_or_else(overflow)?),
            ast::ComparisonOp::Eq => (Cmp::Exactly, bound),
        };
        let value = Amount::fixed(bound);
        atoms.push(match property.property {
            ast::PropertyConstraintProperty::Power => Atom::Power { cmp, value },
            ast::PropertyConstraintProperty::Toughness => Atom::Toughness { cmp, value },
            ast::PropertyConstraintProperty::ManaValue => Atom::ManaValue { cmp, value },
            ref other => {
                return Err(operand("filter", format!("properties:{}", variant(other))));
            }
        });
    }
    Ok(Pred(atoms))
}

/// The filter terms a board set represents.
const BOARD_FILTER_TERMS: [&str; 16] = [
    "cls",
    "types",
    "all_of",
    "non_types",
    "supertypes",
    "non_supertypes",
    "subtypes",
    "non_subtypes",
    "colors",
    "non_colors",
    "control",
    "status",
    "non_token",
    "with_ability",
    "without_ability",
    "properties",
];

/// The sole printed term of a filter, if it has exactly one.
fn only_field(filter: &ast::ObjectFilter) -> Option<&'static str> {
    match present_fields(filter).as_slice() {
        [field] => Some(field),
        _ => None,
    }
}

/// Every printed term of a filter, by field name. Exhaustive over
/// `ast::ObjectFilter`: a field the parser adds fails to compile here until
/// it is named.
fn present_fields(filter: &ast::ObjectFilter) -> Vec<&'static str> {
    let ast::ObjectFilter {
        characteristic_relations,
        all_of,
        damage_history,
        zone_change_history,
        any_of,
        none_of,
        types,
        non_types,
        cls,
        ability_kind,
        mana_ability,
        tap_in_cost,
        loyalty_ability,
        tap_symbol_in_cost,
        ability_keyword,
        ability_source,
        attached_to,
        blocking,
        blocked_by,
        attacking,
        attacking_lookup,
        permanent,
        commander,
        ring_bearer,
        sticker,
        subtypes,
        non_subtypes,
        supertypes,
        non_supertypes,
        colors,
        non_colors,
        colorless,
        multicolored,
        monocolored,
        non_token,
        status,
        non_status,
        combat_history,
        other,
        other_than,
        kicked,
        mana_cost_contains,
        adventure,
        control,
        controlled_by,
        cast,
        cast_for_keyword_cost,
        not_cast_this_way,
        name,
        not_name,
        chosen_type,
        not_chosen_type,
        chosen_color,
        value_of_choice,
        chosen_name,
        own,
        parity,
        owned_by,
        zone,
        outside_game,
        drafted_not_in_deck,
        outside_zone,
        properties,
        implicit_comparison,
        implicit_same_controller,
        targets,
        targets_only,
        single_target,
        or_player,
        or_opponent,
        with_ability,
        without_ability,
        has_abilities,
        counter_constraints,
        relation,
        from_among,
    } = filter;
    let fields: [(&'static str, bool); 76] = [
        (
            "characteristic_relations",
            characteristic_relations.is_some(),
        ),
        ("all_of", all_of.is_some()),
        ("damage_history", damage_history.is_some()),
        ("zone_change_history", zone_change_history.is_some()),
        ("any_of", any_of.is_some()),
        ("none_of", none_of.is_some()),
        ("types", types.is_some()),
        ("non_types", non_types.is_some()),
        ("cls", cls.is_some()),
        ("ability_kind", ability_kind.is_some()),
        ("mana_ability", mana_ability.is_some()),
        ("tap_in_cost", tap_in_cost.is_some()),
        ("loyalty_ability", loyalty_ability.is_some()),
        ("tap_symbol_in_cost", tap_symbol_in_cost.is_some()),
        ("ability_keyword", ability_keyword.is_some()),
        ("ability_source", ability_source.is_some()),
        ("attached_to", attached_to.is_some()),
        ("blocking", blocking.is_some()),
        ("blocked_by", blocked_by.is_some()),
        ("attacking", attacking.is_some()),
        ("attacking_lookup", attacking_lookup.is_some()),
        ("permanent", permanent.is_some()),
        ("commander", commander.is_some()),
        ("ring_bearer", ring_bearer.is_some()),
        ("sticker", sticker.is_some()),
        ("subtypes", subtypes.is_some()),
        ("non_subtypes", non_subtypes.is_some()),
        ("supertypes", supertypes.is_some()),
        ("non_supertypes", non_supertypes.is_some()),
        ("colors", colors.is_some()),
        ("non_colors", non_colors.is_some()),
        ("colorless", colorless.is_some()),
        ("multicolored", multicolored.is_some()),
        ("monocolored", monocolored.is_some()),
        ("non_token", non_token.is_some()),
        ("status", status.is_some()),
        ("non_status", non_status.is_some()),
        ("combat_history", combat_history.is_some()),
        ("other", other.is_some()),
        ("other_than", other_than.is_some()),
        ("kicked", kicked.is_some()),
        ("mana_cost_contains", mana_cost_contains.is_some()),
        ("adventure", adventure.is_some()),
        ("control", control.is_some()),
        ("controlled_by", controlled_by.is_some()),
        ("cast", cast.is_some()),
        ("cast_for_keyword_cost", cast_for_keyword_cost.is_some()),
        ("not_cast_this_way", not_cast_this_way.is_some()),
        ("name", name.is_some()),
        ("not_name", not_name.is_some()),
        ("chosen_type", chosen_type.is_some()),
        ("not_chosen_type", not_chosen_type.is_some()),
        ("chosen_color", chosen_color.is_some()),
        ("value_of_choice", value_of_choice.is_some()),
        ("chosen_name", chosen_name.is_some()),
        ("own", own.is_some()),
        ("parity", parity.is_some()),
        ("owned_by", owned_by.is_some()),
        ("zone", zone.is_some()),
        ("outside_game", outside_game.is_some()),
        ("drafted_not_in_deck", drafted_not_in_deck.is_some()),
        ("outside_zone", outside_zone.is_some()),
        ("properties", properties.is_some()),
        ("implicit_comparison", implicit_comparison.is_some()),
        (
            "implicit_same_controller",
            implicit_same_controller.is_some(),
        ),
        ("targets", targets.is_some()),
        ("targets_only", targets_only.is_some()),
        ("single_target", single_target.is_some()),
        ("or_player", or_player.is_some()),
        ("or_opponent", or_opponent.is_some()),
        ("with_ability", with_ability.is_some()),
        ("without_ability", without_ability.is_some()),
        ("has_abilities", has_abilities.is_some()),
        ("counter_constraints", counter_constraints.is_some()),
        ("relation", relation.is_some()),
        ("from_among", from_among.is_some()),
    ];
    fields
        .into_iter()
        .filter_map(|(field, present)| present.then_some(field))
        .collect()
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

/// CR 601.2c, 115.1: one printed target occurrence: exactly one object,
/// chosen by the controller at announcement, with its selection identity.
/// Returns its filter and selection identity.
fn target_occurrence(what: &ast::ObjectRef) -> Result<(&ast::ObjectFilter, &str), Failure> {
    let ast::ObjectRef::Target(target) = what else {
        return Err(operand("target", format!("ref_{}", variant(what))));
    };
    let ast::ObjectRefTarget {
        filter,
        count,
        up_to,
        group_constraints,
        random,
        chosen_by,
        selection_id,
    } = &**target;
    for (field, present) in [
        ("up_to", up_to.is_some()),
        ("group_constraints", group_constraints.is_some()),
        ("random", random.is_some()),
        ("chosen_by", chosen_by.is_some()),
    ] {
        if present {
            return Err(operand("target", field));
        }
    }
    if !matches!(&**count, ast::Amount::Fixed(n) if n.value == 1) {
        return Err(operand("target", "count"));
    }
    let selection = selection_id
        .as_deref()
        .ok_or_else(|| operand("target", "selection_id"))?;
    Ok((filter, selection))
}

/// CR 601.2c, 608.2b: the permanent criteria of a target. `types` lists
/// alternatives here, whereas `ObjectFilter.types` requires every type
/// (oracle-parser/src/ast.ts), so a printed conjunction rejects rather than
/// widening the criteria (CR 205.2b). CR 110.5: a status term tests the
/// permanent's current tapped status.
pub(crate) fn lower_target_filter(filter: &ast::ObjectFilter) -> Result<PermanentFilter, Failure> {
    const TERMS: [&str; 7] = [
        "cls",
        "types",
        "all_of",
        "non_types",
        "non_colors",
        "control",
        "status",
    ];
    if let Some(field) = present_fields(filter)
        .into_iter()
        .find(|field| !TERMS.contains(field))
    {
        return Err(operand("target_filter", field));
    }
    let mut types = vec![];
    if let Some(printed) = &filter.types {
        if printed.len() > 1 {
            return Err(operand("target_filter", "types:conjunction"));
        }
        for kind in printed {
            types.push(card_kind(kind, "target_filter")?);
        }
    }
    if let Some(all_of) = &filter.all_of {
        if !types.is_empty() {
            return Err(operand("target_filter", "all_of:mixed"));
        }
        types = type_alternatives(all_of, "target_filter")?;
    }
    match &filter.cls {
        None if types.is_empty() => return Err(operand("target_filter", "untyped")),
        None => {}
        Some(ast::ObjectFilterCls::Permanent) if types.is_empty() => {}
        Some(other) => return Err(operand("target_filter", format!("cls:{}", variant(other)))),
    }
    let non_types = filter
        .non_types
        .iter()
        .flatten()
        .map(|kind| card_kind(kind, "target_filter"))
        .collect::<Result<_, _>>()?;
    let non_colors = filter
        .non_colors
        .iter()
        .flatten()
        .map(|c| match c {
            ast::Color::Black => Ok(Color::B),
            other => Err(operand(
                "target_filter",
                format!("non_colors:{}", variant(other)),
            )),
        })
        .collect::<Result<_, _>>()?;
    let opponent = match &filter.control {
        None => false,
        Some(ast::ObjectFilterControl::Opponent) => true,
        Some(other) => {
            return Err(operand(
                "target_filter",
                format!("control:{}", variant(other)),
            ));
        }
    };
    let tapped = match filter.status.as_deref() {
        None => false,
        Some([status]) if status == "tapped" => true,
        Some(_) => return Err(operand("target_filter", "status")),
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

/// "target creature": a target occurrence whose only term is the creature
/// type (CR 601.2c/608.2b); controller, blocking or other qualifiers reject.
fn creature_target(what: &ast::ObjectRef) -> Result<&str, Failure> {
    let (filter, selection) = target_occurrence(what)?;
    if let Some(field) = present_fields(filter)
        .into_iter()
        .find(|field| *field != "types")
    {
        return Err(operand("creature_target", field));
    }
    match filter.types.as_deref() {
        Some([kind]) if kind == "creature" => Ok(selection),
        _ => Err(operand("creature_target", "types")),
    }
}

/// True when the target occurrence is exactly "target creature".
pub(crate) fn is_creature_target(what: &ast::ObjectRef) -> bool {
    creature_target(what).is_ok()
}

/// True when the target occurrence's class is "permanent".
pub(crate) fn is_permanent_target(what: &ast::ObjectRef) -> bool {
    target_occurrence(what)
        .is_ok_and(|(filter, _)| filter.cls == Some(ast::ObjectFilterCls::Permanent))
}

/// True when the target occurrence is exactly "target permanent".
pub(crate) fn is_any_permanent_target(what: &ast::ObjectRef) -> bool {
    target_occurrence(what).is_ok_and(|(filter, _)| {
        present_fields(filter) == ["cls"] && filter.cls == Some(ast::ObjectFilterCls::Permanent)
    })
}

pub(crate) fn bind_creature_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<u32, Failure> {
    let selection = creature_target(what)?;
    bind_target(program, slots, selection, TargetDomain::BattlefieldCreature)
}

pub(crate) fn bind_permanent_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<u32, Failure> {
    let (filter, selection) = target_occurrence(what)?;
    let filter = lower_target_filter(filter)?;
    let slot = bind_target(
        program,
        slots,
        selection,
        TargetDomain::BattlefieldPermanent,
    )?;
    let target = &mut program.targets[slot as usize];
    if target.filter.as_ref().is_some_and(|old| *old != filter) {
        return Err(operand("target_filter", "conflicting"));
    }
    target.filter = Some(filter);
    Ok(slot)
}

/// CR 107.1: a positive fixed number.
pub(crate) fn fixed_positive_amount(amount: &ast::Amount) -> Result<Quantity, Failure> {
    match amount {
        ast::Amount::Fixed(n) if n.value > 0 && u32::try_from(n.value).is_ok() => {
            Ok(Quantity::from(n.value))
        }
        ast::Amount::Fixed(_) => Err(operand("amount", "fixed:range")),
        other => Err(operand("amount", variant(other))),
    }
}

/// A positive fixed count printed in the shared mirror.
pub(crate) fn fixed_positive(count: &Count) -> Result<Quantity, Failure> {
    fixed_positive_amount(&typed(count, "amount")?)
}

/// CR 613.4c: a fixed signed power or toughness change.
pub(crate) fn signed_fixed(value: &SignedAmount) -> Result<Quantity, Failure> {
    let ast::SignedAmount { sign, amount } = typed(value, "signed_amount")?;
    let ast::Amount::Fixed(n) = &*amount else {
        return Err(operand("signed_amount", variant(&*amount)));
    };
    let value = u32::try_from(n.value)
        .map(i64::from)
        .map_err(|_| operand("signed_amount", "fixed:range"))?;
    Ok(Quantity::from(match *sign {
        ast::SignedAmountSign::N1 => value,
        ast::SignedAmountSign::Minus1 => -value,
    }))
}

/// CR 120.3, 107.3, 608.2h: the quantity of a damage step: a positive fixed
/// number, the announced X (or, under a counted binding, the number of
/// creatures on the battlefield), or the total mana value of the other
/// spells you cast this turn.
pub(crate) fn lower_damage_amount(
    amount: &ast::Amount,
    counted: bool,
    context: Context,
) -> Result<kir::Amount, Failure> {
    match amount {
        ast::Amount::X(_) if counted => Ok(kir::Amount::Count {
            set: SetExpr::battlefield(kir::Pred::creatures()),
        }),
        ast::Amount::X(_) => Ok(kir::Amount::announced_x()),
        _ if counted => Err(unsupported("binding", "unused X")),
        ast::Amount::CastManaValueSum(sum) => match (&*sum.who, &*sum.excluding) {
            (ast::PlayerRef::You(_), ast::ObjectRef::This(this)) if this.noun == "spell" => {
                Ok(kir::Amount::OtherCastsManaValue {
                    caster: PlayerExpr::you(context),
                })
            }
            _ => Err(operand("amount", "cast_mana_value_sum:scope")),
        },
        other => Ok(kir::Amount::Fixed {
            value: fixed_positive_amount(other)?,
        }),
    }
}

/// CR 107.3, 608.2h: the sentence binding "where X is the number of
/// creatures on the battlefield", the one X definition a step reads.
pub(crate) fn counts_creatures(bindings: &[serde_json::Value]) -> Result<bool, Failure> {
    let bindings: Vec<ast::SentenceBindingsItem> = typed(&bindings, "bindings")?;
    match bindings.as_slice() {
        [] => Ok(false),
        [
            ast::SentenceBindingsItem {
                value, as_of: None, ..
            },
        ] => match &**value {
            ast::Amount::Count(count)
                if present_fields(&count.of) == ["types", "zone"]
                    && count.of.types.as_deref() == Some(&["creature".to_owned()])
                    && count.of.zone.as_deref().is_some_and(|zone| {
                        zone.zone == ast::Zone::Battlefield
                            && zone.position.is_none()
                            && zone.depth.is_none()
                            && zone.owner.is_none()
                    }) =>
            {
                Ok(true)
            }
            other => Err(operand("bindings", variant(other))),
        },
        _ => Err(operand("bindings", "several")),
    }
}

/// CR 109.5, 119.3: the player a step names. "You" is the controller of
/// this kind of stack object; "that player" is the trigger's event player.
pub(crate) fn lower_player(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    who: &PlayerSelector,
    context: Context,
    event_player: Option<kir::EventPlayerRole>,
) -> Result<PlayerExpr, Failure> {
    match typed::<ast::PlayerRef>(who, "player")? {
        ast::PlayerRef::You(_) => Ok(PlayerExpr::you(context)),
        ast::PlayerRef::ThatPlayer(_) => event_player
            .map(PlayerExpr::event)
            .ok_or_else(|| operand("player", "that_player:unbound")),
        ast::PlayerRef::EachOpponent(_) => Ok(PlayerExpr::EachOpponent),
        ast::PlayerRef::TargetPlayer(target) => match *target {
            ast::PlayerRefTargetPlayer {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } => Ok(PlayerExpr::target(bind_target(
                program,
                slots,
                &id,
                TargetDomain::AnyPlayer,
            )?)),
            _ => Err(operand("player", "target_player:qualified")),
        },
        other => Err(operand("player", variant(&other))),
    }
}

/// CR 400.3, 401.4: a destination zone the move verbs perform.
pub(crate) fn lower_destination(to: &serde_json::Value) -> Result<kir::Destination, Failure> {
    let zone: ast::ZoneRef = typed(to, "destination")?;
    match zone {
        ast::ZoneRef {
            zone: ast::Zone::Library,
            position: Some(ast::ObjectFilterZoneChangeHistoryFromZoneRefPosition::Top),
            depth: None,
            owner: Some(owner),
        } if matches!(
            *owner,
            ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner::ItsOwner
        ) =>
        {
            Ok(kir::Destination::OwnerLibraryTop)
        }
        other => Err(operand("destination", variant(&other.zone))),
    }
}

/// CR 611.2a, 514.2: "until end of turn", the one duration the
/// until-end-of-turn cells execute.
pub(crate) fn until_end_of_turn(duration: &SourceDuration) -> Result<(), Failure> {
    match typed::<ast::Duration>(duration, "duration")? {
        ast::Duration::EndOfTurn(_) => Ok(()),
        other => Err(operand("duration", variant(&other))),
    }
}

/// Decode a printed object operand.
pub(crate) fn object(what: &serde_json::Value) -> Result<ast::ObjectRef, Failure> {
    typed(what, "object")
}

/// CR 201.5: "this creature", the exact source.
pub(crate) fn is_this_creature(what: &ast::ObjectRef) -> bool {
    matches!(what, ast::ObjectRef::This(this) if this.noun == "creature")
}

/// An "all"/"each" set operand.
pub(crate) fn is_set(what: &ast::ObjectRef) -> bool {
    matches!(what, ast::ObjectRef::All(_) | ast::ObjectRef::Each(_))
}

pub(crate) fn is_source_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::This {noun} if noun=="creature")
}

pub(crate) fn is_other_creature(subject: &TriggerObject) -> bool {
    matches!(subject,TriggerObject::Filter {filter,count} if filter.properties.is_none() && filter.other && filter.types==["creature"] && filter.control.is_none() && !filter.non_token && filter.non_subtypes.is_empty() && fixed_one(count))
}
