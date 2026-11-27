//! Operand lowerers: total functions from one typed AST operand (object,
//! player, filter, amount, target) to its IR operand, or an explicit rejection.
use super::*;

pub(crate) fn lower_damage_subject(what: &SourceDamageObject) -> Result<Pred, Failure> {
    match what {
        SourceDamageObject::This { noun } if noun == "creature" => Ok(Pred::source()),
        SourceDamageObject::Filter { filter, count }
            if fixed_one(count)
                && filter.types.iter().any(|t| t == "creature")
                && filter.properties.is_none() =>
        {
            // CR 109.5: a creature "you control" deals the damage.
            if filter.control.as_deref() != Some("you") {
                return Err(unsupported(
                    "trigger.what.filter.control",
                    "unsupported damage source controller",
                ));
            }
            let mut pred = lower_type_predicate(
                &SourceTypeFilter {
                    colors: None,
                    cls: None,
                    types: Some(filter.types.clone()),
                    subtypes: None,
                    non_types: filter.non_types.clone(),
                    all_of: filter.all_of.clone(),
                    any_of: filter.any_of.clone(),
                },
                None,
                true,
                0,
            )?;
            pred.0.push(Atom::ControlledBy {
                player: PlayerExpr::you(Context::Triggered),
            });
            Ok(pred)
        }
        _ => Err(unsupported(
            "trigger.what",
            "unsupported complete creature damage source/cardinality",
        )),
    }
}

/// The type tree of a printed event filter before it becomes a [`Pred`]:
/// fields conjunctive, `allOf` conjunctive and `anyOf` disjunctive
/// (packages/oracle-parser/src/ast.ts ObjectFilter; CR 205.2b, 601.2i).
enum TypeTerm {
    /// The empty conjunction: any object.
    Any,
    Color(Color),
    Type(CardKind),
    Subtype(String),
    NotType(CardKind),
    All(Vec<TypeTerm>),
    AnyOf(Vec<TypeTerm>),
}

impl TypeTerm {
    /// The structural bound of a printed type predicate: depth 8 and 32
    /// terms in all; a color stands alone at the root.
    fn bounded(&self) -> bool {
        fn visit(t: &TypeTerm, depth: usize, remaining: &mut usize, root: bool) -> bool {
            if depth > 8 || *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            match t {
                TypeTerm::Color(_) => root,
                TypeTerm::All(terms) | TypeTerm::AnyOf(terms) => {
                    !terms.is_empty() && terms.iter().all(|t| visit(t, depth + 1, remaining, false))
                }
                _ => true,
            }
        }
        visit(self, 0, &mut 32, true)
    }
    fn atoms(self, out: &mut Vec<Atom>) {
        match self {
            Self::Any => {}
            Self::Color(color) => out.push(Atom::Color { color }),
            Self::Subtype(subtype) => out.push(Atom::Subtype { subtype }),
            Self::Type(kind) => out.push(Atom::Type { kind }),
            Self::NotType(kind) => out.push(Atom::NotType { kind }),
            Self::All(terms) => terms.into_iter().for_each(|t| t.atoms(out)),
            Self::AnyOf(terms) => out.push(Atom::AnyOf {
                alternatives: terms
                    .into_iter()
                    .map(|t| {
                        let mut alternative = vec![];
                        t.atoms(&mut alternative);
                        Pred(alternative)
                    })
                    .collect(),
            }),
        }
    }
}

/// CR 205.2b, 601.2i: the type and color terms of an event's filtered
/// object as one predicate, its fields conjunctive and `anyOf` an
/// alternative.
pub(crate) fn lower_type_predicate(
    f: &SourceTypeFilter,
    class: Option<&str>,
    root: bool,
    depth: usize,
) -> Result<Pred, Failure> {
    let term = type_term(f, class, root, depth)?;
    if !term.bounded() {
        return Err(unsupported(
            "trigger.what",
            "type predicate structural bound",
        ));
    }
    let mut atoms = vec![];
    term.atoms(&mut atoms);
    Ok(Pred(atoms))
}

/// The type tree of one filter.
fn type_term(
    f: &SourceTypeFilter,
    class: Option<&str>,
    root: bool,
    depth: usize,
) -> Result<TypeTerm, Failure> {
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
            || f.subtypes.is_some()
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
        return Ok(TypeTerm::Color(color));
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
    if let Some(subtypes) = &f.subtypes {
        if class != Some("spell") || subtypes.is_empty() {
            return Err(unsupported(
                "trigger.what.subtypes",
                "unsupported spell subtype",
            ));
        }
        terms.extend(subtypes.iter().cloned().map(TypeTerm::Subtype));
    }
    for (values, negative) in [(&f.types, false), (&f.non_types, true)] {
        if let Some(values) = values {
            if values.is_empty() {
                return Err(unsupported("trigger.what", "empty type predicate"));
            }
            for t in values {
                let kind = kind(t)?;
                terms.push(if negative {
                    TypeTerm::NotType(kind)
                } else {
                    TypeTerm::Type(kind)
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
                .map(|c| type_term(c, class, false, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            terms.push(if disjunction {
                TypeTerm::AnyOf(children)
            } else {
                TypeTerm::All(children)
            });
        }
    }
    Ok(match terms.len() {
        0 => TypeTerm::Any,
        1 => terms.remove(0),
        _ => TypeTerm::All(terms),
    })
}

/// CR 508.3a: the attacking creature an attack event names: this creature
/// or a typed creature by its controller's relation to "you".
pub(crate) fn lower_attack_subject(what: &SourceAttacker) -> Result<Pred, Failure> {
    match what {
        SourceAttacker::This { noun } if noun == "creature" => Ok(Pred::source()),
        SourceAttacker::Filter { filter, count } if fixed_one(count) => {
            let controller = match filter.control.as_deref() {
                None => None,
                Some("you") => Some(PlayerExpr::you(Context::Triggered)),
                Some("opponent") => Some(PlayerExpr::EachOpponent),
                _ => {
                    return Err(unsupported(
                        "trigger.what.filter.control",
                        "unsupported attacker controller",
                    ));
                }
            };
            let mut pred = lower_type_predicate(
                &SourceTypeFilter {
                    colors: None,
                    cls: None,
                    types: filter.types.clone(),
                    subtypes: None,
                    non_types: filter.non_types.clone(),
                    all_of: filter.all_of.clone(),
                    any_of: filter.any_of.clone(),
                },
                None,
                true,
                0,
            )?;
            if let Some(player) = controller {
                pred.0.push(Atom::ControlledBy { player });
            }
            Ok(pred)
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
    let battlefield_creature_route = |domain| {
        matches!(
            domain,
            TargetDomain::BattlefieldCreature
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent
        )
    };
    // Fight's separate creature slots share the object announcement frame
    // but can use different filters. Definition admission limits that mix
    // to the audited fight program.
    if id.is_empty()
        || program.targets.iter().any(|target| {
            target.domain != domain
                && !(battlefield_creature_route(domain)
                    && battlefield_creature_route(target.domain))
        })
    {
        return Err(unsupported(
            "targets",
            "empty selection or mixed creature/player target domains",
        ));
    }
    if let Some(slot) = slots.get(id) {
        if program.targets[*slot as usize].domain != domain {
            return Err(unsupported(
                "targets",
                "conflicting target occurrence domain",
            ));
        }
        return Ok(*slot);
    }
    let slot = u32::try_from(program.targets.len())
        .map_err(|_| invalid("targets", "structural slot limit"))?;
    program.targets.push(TargetSpec::new(slot, id, domain));
    slots.insert(id.into(), slot);
    Ok(slot)
}

/// What an ability's resolving instructions may refer back to besides their
/// own steps: the trigger's event bindings (CR 603.2, 608.2h), whether the
/// event object is the source itself (CR 201.5), and what earlier steps of
/// the same block, lowered outside this call, already introduced.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) player: PlayerBinding,
    pub(crate) event_is_source: bool,
    /// The card types the trigger's event object pattern names, as
    /// [`CardKind::CARD_TYPES`] bits; zero names none.
    pub(crate) event_types: u8,
    pub(crate) prior: crate::ingest::Introduced,
}

impl From<PlayerBinding> for Scope {
    fn from(player: PlayerBinding) -> Self {
        Self {
            player,
            event_is_source: false,
            event_types: 0,
            prior: Default::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerBinding {
    Unbound,
    /// "That player" is the trigger's event player in this role.
    EventPlayer(kir::EventPlayerRole, Option<kir::EventObjectRole>),
    EventObject(kir::EventObjectRole),
    DamageRecipient,
    DeathController,
    LifeGain,
}

impl PlayerBinding {
    pub(crate) fn event_role(self) -> Option<kir::EventPlayerRole> {
        match self {
            Self::EventPlayer(role, _) => Some(role),
            Self::DamageRecipient => Some(kir::EventPlayerRole::DamagedPlayer),
            _ => None,
        }
    }

    pub(crate) fn supplies_amount(self) -> bool {
        matches!(
            self,
            Self::DamageRecipient
                | Self::LifeGain
                | Self::EventObject(kir::EventObjectRole::Damaged)
        )
    }

    pub(crate) fn event_object_role(self) -> Option<kir::EventObjectRole> {
        match self {
            Self::EventPlayer(_, role) => role,
            Self::EventObject(role) => Some(role),
            Self::DeathController => Some(kir::EventObjectRole::Died),
            Self::DamageRecipient => Some(kir::EventObjectRole::DealtDamage),
            Self::Unbound | Self::LifeGain => None,
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
/// decoded as the complete AST, so a failure here is an archive defect. The
/// generated `Option` fields read an explicit `null` as absent; a printed
/// operand never spells a term as `null`, so one anywhere in the operand
/// rejects it rather than silently dropping that term.
pub(crate) fn typed<T: serde::de::DeserializeOwned>(
    value: &impl serde::Serialize,
    kind: &str,
) -> Result<T, Failure> {
    let value = serde_json::to_value(value)
        .map_err(|e| invalid("operand", format!("operand:{kind}: {e}")))?;
    if let Some(field) = null_field(&value, kind) {
        return Err(operand(kind, format!("null:{field}")));
    }
    serde_json::from_value(value).map_err(|e| invalid("operand", format!("operand:{kind}: {e}")))
}

/// The field of the first `null` at any depth: an object member whose value
/// is `null`, or a `null` element of an array, named by the array's field
/// (`enclosing` for a top-level array, or for a root that is itself `null`).
pub(crate) fn null_field(value: &serde_json::Value, enclosing: &str) -> Option<String> {
    match value {
        serde_json::Value::Null => Some(enclosing.to_owned()),
        serde_json::Value::Object(members) => members.iter().find_map(|(key, member)| {
            if member.is_null() {
                Some(key.clone())
            } else {
                null_field(member, key)
            }
        }),
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            if item.is_null() {
                Some(enclosing.to_owned())
            } else {
                null_field(item, enclosing)
            }
        }),
        _ => None,
    }
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
    let mut set = lower_board_set(what, context)?;
    // CR 205.3m, 308.2: a positive creature subtype also identifies a
    // creature here, because no admitted card or effect adds the Kindred
    // type. This is the controlled-subtype form of a fixed pump.
    if let SetExpr::Matching { filter, .. } = &mut set
        && let [Atom::Subtype { subtype }, Atom::ControlledBy { player }] = filter.0.as_slice()
        && kir::is_creature_subtype(subtype)
        && *player == PlayerExpr::you(context)
    {
        filter.0.insert(
            0,
            Atom::Type {
                kind: CardKind::Creature,
            },
        );
        return Ok(set);
    }
    if !REVIEWED
        .iter()
        .any(|reviewed| reviewed.kernel(context) == set)
    {
        return Err(operand("set", "unreviewed_set"));
    }
    Ok(set)
}

/// CR 205.2a: a printed card type, by its AST spelling.
pub(crate) fn card_kind(kind: &str, name: &str) -> Result<CardKind, Failure> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned()))
        .map_err(|_| operand(name, format!("type:{kind}")))
}

/// CR 105.1: the five colors.
pub(crate) fn color(color: &ast::Color) -> Color {
    match color {
        ast::Color::White => Color::W,
        ast::Color::Blue => Color::U,
        ast::Color::Black => Color::B,
        ast::Color::Red => Color::R,
        ast::Color::Green => Color::G,
    }
}

/// A printed list term: present lists are never empty.
pub(crate) fn listed<'a, T>(list: &'a Option<Vec<T>>, field: &str) -> Result<&'a [T], Failure> {
    match list {
        None => Ok(&[]),
        Some(items) if items.is_empty() => Err(operand("filter", format!("{field}:empty"))),
        Some(items) => Ok(items),
    }
}

/// "artifact or enchantment": one `allOf` group of single-type alternatives.
pub(crate) fn type_alternatives(
    all_of: &[ast::ObjectFilter],
    kind: &str,
) -> Result<Vec<CardKind>, Failure> {
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
    lower_filter_for(filter, PlayerExpr::you(context))
}

fn lower_filter_for(filter: &ast::ObjectFilter, you: PlayerExpr) -> Result<Pred, Failure> {
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
        // CR 109.5: "you control" uses the binding for this ability.
        Some(ast::ObjectFilterControl::You) => atoms.push(Atom::ControlledBy { player: you }),
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
    for property in listed(&filter.properties, "properties")? {
        atoms.push(lower_fixed_property(property)?);
    }
    Ok(Pred(atoms))
}

/// CR 208.1, 202.3: a fixed comparison of power, toughness or mana value.
pub(crate) fn lower_fixed_property(property: &ast::PropertyConstraint) -> Result<Atom, Failure> {
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
    Ok(match property.property {
        ast::PropertyConstraintProperty::Power => Atom::Power { cmp, value },
        ast::PropertyConstraintProperty::Toughness => Atom::Toughness { cmp, value },
        ast::PropertyConstraintProperty::ManaValue => Atom::ManaValue { cmp, value },
        ref other => {
            return Err(operand("filter", format!("properties:{}", variant(other))));
        }
    })
}

/// CR 205, 208, 202.3: the printed characteristics a library card is
/// searched for: card types, supertypes, subtypes, colors and fixed
/// comparisons, their conjunctions ("all of") and alternatives ("any of").
/// A card has no controller, status or permanent keywords in a library.
pub(crate) fn lower_card_filter(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    let mut plain = filter.clone();
    let each = plain.all_of.take();
    let any = plain.any_of.take();
    match plain.cls.take() {
        None | Some(ast::ObjectFilterCls::Card) => {}
        Some(other) => return Err(operand("filter", format!("cls:{}", variant(&other)))),
    }
    if let Some(field) = present_fields(&plain).into_iter().find(|field| {
        matches!(
            *field,
            "control" | "status" | "non_token" | "with_ability" | "without_ability"
        )
    }) {
        return Err(operand("filter", format!("library_card:{field}")));
    }
    // Context only names "you" for a control atom, which is rejected above.
    let mut pred = lower_filter_for(&plain, PlayerExpr::you(Context::Spell))?;
    if let Some(children) = each {
        if children.is_empty() {
            return Err(operand("filter", "all_of:empty"));
        }
        for child in &children {
            pred.0.extend(lower_card_filter(child)?.0);
        }
    }
    if let Some(children) = any {
        if children.is_empty() {
            return Err(operand("filter", "any_of:empty"));
        }
        let alternatives = children
            .iter()
            .map(lower_card_filter)
            .collect::<Result<_, _>>()?;
        pred.0.push(Atom::AnyOf { alternatives });
    }
    Ok(pred)
}

/// CR 109.5, 201.5: bind a live static predicate, including "other".
pub(crate) fn lower_static_filter(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    let mut plain = filter.clone();
    let each = plain.all_of.take();
    let any = plain.any_of.take();
    let other = plain.other.take();
    if other == Some(false) {
        return Err(operand("filter", "other:false"));
    }
    let mut pred = lower_filter_for(&plain, PlayerExpr::static_controller())?;
    if let Some(children) = each {
        if children.is_empty() {
            return Err(operand("filter", "all_of:empty"));
        }
        for child in &children {
            pred.0.extend(lower_static_filter(child)?.0);
        }
    }
    if let Some(children) = any {
        if children.is_empty() {
            return Err(operand("filter", "any_of:empty"));
        }
        let alternatives = children
            .iter()
            .map(lower_static_filter)
            .collect::<Result<_, _>>()?;
        pred.0.push(Atom::AnyOf { alternatives });
    }
    if other.is_some() {
        pred.0.push(Atom::IsNot {
            object: kir::Ref::Source,
        });
    }
    Ok(pred)
}

/// CR 509.1b, 613.11: a combat restriction's object filter, read from the
/// current object at each declaration, so any keyword may be tested.
pub(crate) fn lower_declaration_filter(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    let mut plain = filter.clone();
    let with = plain.with_ability.take();
    let without = plain.without_ability.take();
    let each = plain.all_of.take();
    let any = plain.any_of.take();
    if plain.other.is_some() {
        return Err(operand("filter", "declaration:other"));
    }
    // "Walls and/or creatures with flying": a postmodifier after noun
    // alternatives may name only the last noun, so the parse is ambiguous.
    let nouns = |f: &ast::ObjectFilter| {
        f.any_of.as_ref().is_some_and(|alternatives| {
            alternatives.iter().any(|a| {
                present_fields(a)
                    .iter()
                    .all(|f| matches!(*f, "types" | "subtypes"))
            })
        })
    };
    let postmodified = with.is_some() || without.is_some() || plain.properties.is_some();
    if postmodified && (each.iter().flatten().any(nouns) || nouns(filter)) {
        return Err(operand("filter", "declaration:postmodified_alternatives"));
    }
    let mut pred = lower_filter_for(&plain, PlayerExpr::static_controller())?;
    for (list, field, has) in [
        (&with, "with_ability", true),
        (&without, "without_ability", false),
    ] {
        for ability in listed(list, field)? {
            let name = match ability.as_str() {
                "first-strike" => "first strike",
                "double-strike" => "double strike",
                other => other,
            };
            let keyword = serde_json::from_value::<KeywordAbility>(serde_json::json!(name))
                .map_err(|_| operand("filter", format!("{field}:{ability}")))?;
            pred.0.push(if has {
                Atom::Keyword { keyword }
            } else {
                Atom::NotKeyword { keyword }
            });
        }
    }
    for child in each.iter().flatten() {
        pred.0.extend(lower_declaration_filter(child)?.0);
    }
    if let Some(children) = any {
        if children.is_empty() {
            return Err(operand("filter", "any_of:empty"));
        }
        let alternatives = children
            .iter()
            .map(lower_declaration_filter)
            .collect::<Result<_, _>>()?;
        pred.0.push(Atom::AnyOf { alternatives });
    }
    if each.as_ref().is_some_and(Vec::is_empty) {
        return Err(operand("filter", "all_of:empty"));
    }
    Ok(pred)
}

/// CR 603.2, 109.5, 201.5: the object a trigger event names, relative to
/// the ability's controller; "other" is not the source itself.
pub(crate) fn lower_event_filter(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    let mut plain = filter.clone();
    let each = plain.all_of.take();
    let any = plain.any_of.take();
    let other = plain.other.take();
    if other == Some(false) {
        return Err(operand("filter", "other:false"));
    }
    let mut pred = lower_filter_for(&plain, PlayerExpr::you(Context::Triggered))?;
    if let Some(children) = each {
        if children.is_empty() {
            return Err(operand("filter", "all_of:empty"));
        }
        for child in &children {
            pred.0.extend(lower_event_filter(child)?.0);
        }
    }
    if let Some(children) = any {
        if children.is_empty() {
            return Err(operand("filter", "any_of:empty"));
        }
        let alternatives = children
            .iter()
            .map(lower_event_filter)
            .collect::<Result<_, _>>()?;
        pred.0.push(Atom::AnyOf { alternatives });
    }
    if other.is_some() {
        pred.0.push(Atom::IsNot {
            object: kir::Ref::Source,
        });
    }
    Ok(pred)
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
pub(crate) fn present_fields(filter: &ast::ObjectFilter) -> Vec<&'static str> {
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
    // CR 111.3-111.4: every printed characteristic is lowered or the
    // recipe rejects under the modifier's name.
    for (field, present) in [
        ("name", token.name.is_some()),
        ("abilities", token.abilities.is_some()),
        ("attacking", token.attacking.is_some()),
        ("attacking_defender", token.attacking_defender.is_some()),
        ("blocking", token.blocking.is_some()),
    ] {
        if present {
            return Err(operand("token", field));
        }
    }
    let stat = |value: &serde_json::Value| match typed::<ast::Amount>(value, "token")? {
        ast::Amount::Fixed(n) => u32::try_from(n.value)
            .map(i64::from)
            .map_err(|_| operand("token_stat", "fixed:range")),
        other => Err(operand("token_stat", variant(&other))),
    };
    if !token.supertypes.is_empty() {
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
    let (Some(power), Some(toughness)) = (&token.power, &token.toughness) else {
        return Err(operand("token_stat", "missing_power_or_toughness"));
    };
    let (power, toughness) = (stat(power)?, stat(toughness)?);
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
    // CR 702.73a: a token with changeling is every creature type.
    let changeling = serde_json::json!({"keyword": "changeling"});
    let all_creature_types = match token.keywords.iter().filter(|k| **k == changeling).count() {
        0 => false,
        1 => true,
        _ => {
            return Err(unsupported("effects.token.keywords", "repeated changeling"));
        }
    };
    let keywords = token
        .keywords
        .iter()
        .filter(|k| **k != changeling)
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
        all_creature_types,
        power: power.into(),
        toughness: toughness.into(),
        keywords,
    };
    token
        .validate()
        .map_err(|e| unsupported("effects.token", e))?;

    Ok(token)
}

/// CR 115.1d, 601.2c: one printed target occurrence. The controller
/// announces one object, or zero if the text allows up to one.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn target_occurrence(
    what: &ast::ObjectRef,
) -> Result<(&ast::ObjectFilter, &str, bool), Failure> {
    let (filter, selection, count, optional) = object_target_occurrence(what)?;
    if count != 1 {
        return Err(operand("target", if optional { "up_to" } else { "count" }));
    }
    Ok((filter, selection, optional))
}

fn object_target_occurrence(
    what: &ast::ObjectRef,
) -> Result<(&ast::ObjectFilter, &str, u32, bool), Failure> {
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
        ("up_to", *up_to == Some(false)),
        ("group_constraints", group_constraints.is_some()),
        ("random", random.is_some()),
        ("chosen_by", chosen_by.is_some()),
    ] {
        if present {
            return Err(operand("target", field));
        }
    }
    let count = match &**count {
        ast::Amount::Fixed(n) => u32::try_from(n.value).ok().filter(|n| *n > 0),
        _ => None,
    }
    .ok_or_else(|| operand("target", if up_to.is_some() { "up_to" } else { "count" }))?;
    let selection = selection_id
        .as_deref()
        .ok_or_else(|| operand("target", "selection_id"))?;
    Ok((filter, selection, count, *up_to == Some(true)))
}

/// CR 115.3, 601.2c: one up-to occurrence with one exact filter. Slot zero
/// keeps the archive's selection identity for resolution graph verification.
pub(crate) fn bind_optional_target_group(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<Option<u32>, Failure> {
    let Ok((filter, selection, count, true)) = object_target_occurrence(what) else {
        return Ok(None);
    };
    if count == 1 {
        return Ok(None);
    }
    if !(2..=7).contains(&count) || !program.targets.is_empty() {
        return Err(operand("target", "up_to"));
    }
    let (domain, filter) = lower_target_filter(filter)?;
    for index in 0..count {
        let identity = if index == 0 {
            selection.to_owned()
        } else {
            format!("{selection}/{index}")
        };
        let slot = bind_target(program, slots, &identity, domain)?;
        program.targets[slot as usize].filter = Some(filter.clone());
    }
    program.optional_target = true;
    Ok(Some(count))
}

/// CR 601.2c, 608.2b: the permanent criteria of a target. `types` lists
/// alternatives, whereas `ObjectFilter.types` requires every type.
/// A printed conjunction keeps one type in the list and adds predicate
/// atoms for the rest (CR 205.2b). Every other term becomes a
/// predicate conjunct, rechecked with the rest at resolution (CR 608.2b).
/// "Another" (CR 109.2, 201.5) excludes the targeting source; admission
/// limits it to a permanent's ability with one target occurrence.
fn lower_target_filter(
    filter: &ast::ObjectFilter,
) -> Result<(TargetDomain, PermanentFilter), Failure> {
    const TERMS: [&str; 18] = [
        "cls",
        "types",
        "all_of",
        "any_of",
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
        "other",
    ];
    if let Some(field) = present_fields(filter)
        .into_iter()
        .find(|field| !TERMS.contains(field))
    {
        return Err(operand("target_filter", field));
    }
    let mut atoms = vec![];
    let mut types = vec![];
    // CR 205.2b: "artifact creature" requires both types. The alternative
    // list keeps one of them (creature when printed) and the rest conjoin.
    let mut conjoined = vec![];
    if filter.types.is_some() {
        let printed = listed(&filter.types, "types")?;
        let mut kinds = printed
            .iter()
            .map(|kind| card_kind(kind, "target_filter"))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(first) = kinds.iter().position(|k| *k == CardKind::Creature) {
            kinds.swap(0, first);
        }
        let mut kinds = kinds.into_iter();
        types.extend(kinds.next());
        conjoined.extend(kinds.map(|kind| Atom::Type { kind }));
    }
    if !conjoined.is_empty() && filter.all_of.is_some() {
        return Err(operand("target_filter", "types:conjunction"));
    }
    atoms.append(&mut conjoined);
    if let Some(all_of) = &filter.all_of {
        target_alternative_groups(all_of, &mut types, &mut atoms, 0)?;
    }
    if let Some(alternatives) = &filter.any_of {
        lower_target_alternatives(alternatives, &mut types, &mut atoms)?;
    }
    let token = match &filter.cls {
        None => false,
        Some(ast::ObjectFilterCls::Permanent) if types.is_empty() => false,
        Some(ast::ObjectFilterCls::Token) => true,
        Some(other) => return Err(operand("target_filter", format!("cls:{}", variant(other)))),
    };
    let non_types = filter
        .non_types
        .iter()
        .flatten()
        .map(|kind| card_kind(kind, "target_filter"))
        .collect::<Result<_, _>>()?;
    let non_colors = filter.non_colors.iter().flatten().map(color).collect();
    // CR 109.5, 601.2c, 608.2b: the invocation's controller supplies "you"
    // both when announcing a target and when rechecking it.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let (domain, opponent) = match &filter.control {
        None => (TargetDomain::BattlefieldPermanent, false),
        Some(ast::ObjectFilterControl::Opponent) => (TargetDomain::BattlefieldPermanent, true),
        Some(ast::ObjectFilterControl::You) => {
            (TargetDomain::ControlledBattlefieldPermanent, false)
        }
        Some(other) => {
            return Err(operand(
                "target_filter",
                format!("control:{}", variant(other)),
            ));
        }
    };
    let mut tapped = false;
    for status in listed(&filter.status, "status")? {
        match status_atom(status)? {
            Atom::Tapped => tapped = true,
            atom => atoms.push(atom),
        }
    }
    let mut properties = listed(&filter.properties, "properties")?
        .iter()
        .map(lower_fixed_property)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| operand("target_filter", "properties"))?;
    // Stat bounds keep their place at the front of the predicate.
    properties.append(&mut atoms);
    let mut atoms = properties;
    target_characteristic_atoms(filter, &mut atoms)?;
    if token {
        atoms.push(Atom::Token);
    }
    match filter.other {
        None => {}
        Some(true) => atoms.push(Atom::IsNot {
            object: kir::Ref::Source,
        }),
        Some(false) => return Err(operand("target_filter", "other:false")),
    }
    // CR 205.3d: only a creature has creature types; kindred is not an
    // admitted card type, so a creature subtype alone names a creature.
    if types.is_empty()
        && listed(&filter.subtypes, "subtypes")?
            .iter()
            .any(|subtype| kir::is_creature_subtype(subtype))
    {
        types.push(CardKind::Creature);
    }
    // CR 115.4: an untyped "another target" is a damage recipient.
    if types.is_empty()
        && filter.cls.is_none()
        && filter.subtypes.is_none()
        && filter.all_of.is_none()
        && filter.any_of.is_none()
    {
        return Err(operand("target_filter", "untyped"));
    }
    Ok((
        domain,
        PermanentFilter {
            power: None,
            predicate: (!atoms.is_empty()).then_some(Pred(atoms)),
            types,
            non_types,
            non_colors,
            opponent,
            tapped,
        },
    ))
}

/// CR 205.2b, 608.2b: the `allOf` groups of a target filter. One group of
/// single card types gives the type alternatives; every other group of
/// alternatives is one `AnyOf` conjunct. Planeswalker and battle faces are
/// not admitted card types (see mechanics), so beside an admitted type
/// those alternatives name no object (CR 115.1, 306, 310).
fn target_alternative_groups(
    groups: &[ast::ObjectFilter],
    types: &mut Vec<CardKind>,
    atoms: &mut Vec<Atom>,
    depth: usize,
) -> Result<(), Failure> {
    if groups.is_empty() || depth > 2 {
        return Err(operand("target_filter", "all_of:group"));
    }
    for group in groups {
        match (
            only_field(group),
            group.any_of.as_deref(),
            group.all_of.as_deref(),
        ) {
            (Some("all_of"), _, Some(inner)) => {
                target_alternative_groups(inner, types, atoms, depth + 1)?
            }
            (Some("any_of"), Some(alternatives), _) => {
                lower_target_alternatives(alternatives, types, atoms)?
            }
            _ => return Err(operand("target_filter", "all_of:group")),
        }
    }
    Ok(())
}

fn lower_target_alternatives(
    alternatives: &[ast::ObjectFilter],
    types: &mut Vec<CardKind>,
    atoms: &mut Vec<Atom>,
) -> Result<(), Failure> {
    if alternatives.len() < 2 {
        return Err(operand("target_filter", "any_of:arity"));
    }
    fn single_type(a: &ast::ObjectFilter) -> Option<&str> {
        match (only_field(a), a.types.as_deref()) {
            (Some("types"), Some([kind])) => Some(kind.as_str()),
            _ => None,
        }
    }
    if alternatives.iter().all(|a| single_type(a).is_some()) {
        if !types.is_empty() {
            return Err(operand("target_filter", "all_of:mixed"));
        }
        let mut kinds = vec![];
        for kind in alternatives.iter().filter_map(single_type) {
            match kind {
                "battle" => {}
                kind => kinds.push(card_kind(kind, "target_filter")?),
            }
        }
        if kinds.is_empty() {
            return Err(operand("target_filter", "all_of:unadmitted_types"));
        }
        *types = kinds;
    } else {
        atoms.push(Atom::AnyOf {
            alternatives: alternatives
                .iter()
                .map(target_alternative)
                .collect::<Result<_, _>>()?,
        });
    }
    Ok(())
}

/// One alternative of a printed "or" inside a target filter: a single card
/// type, status, color, subtype or supertype term (CR 105, 205, 506.4,
/// 509.1g).
fn target_alternative(filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
    if let Some(field) = present_fields(filter).into_iter().find(|field| {
        !matches!(
            *field,
            "types" | "status" | "colors" | "subtypes" | "supertypes"
        )
    }) {
        return Err(operand("target_filter", format!("any_of:{field}")));
    }
    let mut atoms = vec![];
    for kind in listed(&filter.types, "types")? {
        atoms.push(Atom::Type {
            kind: card_kind(kind, "target_filter")?,
        });
    }
    for status in listed(&filter.status, "status")? {
        atoms.push(status_atom(status)?);
    }
    target_characteristic_atoms(filter, &mut atoms)?;
    if atoms.is_empty() {
        return Err(operand("target_filter", "any_of:alternative"));
    }
    Ok(Pred(atoms))
}

/// CR 110.5, 506.4, 509.1g: a permanent's current status term.
fn status_atom(status: &str) -> Result<Atom, Failure> {
    Ok(match status {
        "tapped" => Atom::Tapped,
        "untapped" => Atom::Untapped,
        "attacking" => Atom::Attacking,
        "blocking" => Atom::Blocking,
        other => return Err(operand("target_filter", format!("status:{other}"))),
    })
}

/// Supertype, subtype, color, token and keyword terms, each one conjunct
/// (CR 105, 111.1, 205.3-4, 702).
fn target_characteristic_atoms(
    filter: &ast::ObjectFilter,
    atoms: &mut Vec<Atom>,
) -> Result<(), Failure> {
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
                other => return Err(operand("target_filter", format!("{field}:{other}"))),
            });
        }
    }
    for (list, field, has) in [
        (&filter.subtypes, "subtypes", true),
        (&filter.non_subtypes, "non_subtypes", false),
    ] {
        for subtype in listed(list, field)? {
            if subtype.is_empty()
                || (mtg_contracts::unsupported_rules_subtype(subtype) && subtype != "Saga")
            {
                return Err(operand("target_filter", format!("{field}:{subtype}")));
            }
            let subtype = subtype.clone();
            atoms.push(if has {
                Atom::Subtype { subtype }
            } else {
                Atom::NotSubtype { subtype }
            });
        }
    }
    for c in listed(&filter.colors, "colors")? {
        atoms.push(Atom::Color { color: color(c) });
    }
    match filter.non_token {
        None => {}
        Some(true) => atoms.push(Atom::NotToken),
        Some(false) => return Err(operand("target_filter", "non_token:false")),
    }
    for (list, field, has) in [
        (&filter.with_ability, "with_ability", true),
        (&filter.without_ability, "without_ability", false),
    ] {
        for ability in listed(list, field)? {
            let name = match ability.as_str() {
                "first-strike" => "first strike",
                "double-strike" => "double strike",
                other => other,
            };
            let keyword = serde_json::from_value::<KeywordAbility>(serde_json::json!(name))
                .ok()
                .filter(|keyword| kir::target_filter_keyword(*keyword))
                .ok_or_else(|| operand("target_filter", format!("{field}:{ability}")))?;
            atoms.push(if has {
                Atom::Keyword { keyword }
            } else {
                Atom::NotKeyword { keyword }
            });
        }
    }
    Ok(())
}

/// "target creature": a target occurrence whose only term is the creature
/// type (CR 601.2c/608.2b); controller, blocking or other qualifiers reject.
fn creature_target(what: &ast::ObjectRef) -> Result<(&str, bool), Failure> {
    let (filter, selection, optional) = target_occurrence(what)?;
    if let Some(field) = present_fields(filter)
        .into_iter()
        .find(|field| *field != "types")
    {
        return Err(operand("creature_target", field));
    }
    match filter.types.as_deref() {
        Some([kind]) if kind == "creature" => Ok((selection, optional)),
        _ => Err(operand("creature_target", "types")),
    }
}

/// True when the target occurrence is exactly "target creature".
pub(crate) fn is_creature_target(what: &ast::ObjectRef) -> bool {
    creature_target(what).is_ok()
}

pub(crate) fn bind_creature_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<u32, Failure> {
    if let Ok((selection, optional)) = creature_target(what) {
        return bind_object_target(
            program,
            slots,
            selection,
            TargetDomain::BattlefieldCreature,
            optional,
        );
    }
    // CR 601.2c, 608.2b: a qualified "target creature" is a permanent
    // target whose criteria prove a creature.
    let (filter, selection, optional) = target_occurrence(what)?;
    let (domain, mut lowered) = lower_target_filter(filter)?;
    // CR 205.3d: every arm of a creature-subtype alternative requires a
    // creature in the admitted type universe (which excludes kindred).
    if lowered.types.is_empty() && lowered.predicate.as_ref().is_some_and(requires_creature) {
        lowered.types.push(CardKind::Creature);
    }
    if lowered.types != [CardKind::Creature] {
        return Err(creature_target(what).expect_err("not a plain creature target"));
    }
    bind_filtered_object_target(program, slots, selection, domain, optional, lowered)
}

fn requires_creature(predicate: &Pred) -> bool {
    predicate.0.iter().any(|atom| match atom {
        Atom::Subtype { subtype } => kir::is_creature_subtype(subtype),
        Atom::AnyOf { alternatives } => alternatives.iter().all(requires_creature),
        _ => false,
    })
}

/// A damage recipient named by one target occurrence: "target creature" or,
/// CR 115.1, 120.3: "target player or planeswalker" is an entity domain.
/// An "up to" player occurrence rejects.
pub(crate) fn bind_damage_object_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<u32, Failure> {
    if let Ok((filter, selection, false)) = target_occurrence(what)
        && filter.or_player == Some(true)
        && present_fields(filter) == ["types", "or_player"]
        && matches!(filter.types.as_deref(), Some([kind]) if kind == "planeswalker")
    {
        return bind_target(
            program,
            slots,
            selection,
            TargetDomain::PlayerOrPlaneswalker,
        );
    }
    // CR 120.3c,e: a permanent damage target can be a creature or a
    // planeswalker. Preserve the full filter, including color qualifiers.
    if let Ok((filter, selection, optional)) = target_occurrence(what)
        && let Ok((domain, lowered)) = lower_target_filter(filter)
        && lowered.types.contains(&CardKind::Planeswalker)
        && lowered
            .types
            .iter()
            .all(|kind| matches!(kind, CardKind::Creature | CardKind::Planeswalker))
    {
        return bind_filtered_object_target(program, slots, selection, domain, optional, lowered);
    }
    bind_creature_target(program, slots, what)
}

pub(crate) fn bind_permanent_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
) -> Result<u32, Failure> {
    let (filter, selection, optional) = target_occurrence(what)?;
    let (domain, filter) = lower_target_filter(filter)?;
    bind_filtered_object_target(program, slots, selection, domain, optional, filter)
}

fn bind_filtered_object_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    selection: &str,
    domain: TargetDomain,
    optional: bool,
    filter: PermanentFilter,
) -> Result<u32, Failure> {
    let slot = bind_object_target(program, slots, selection, domain, optional)?;
    let target = &mut program.targets[slot as usize];
    if target.filter.as_ref().is_some_and(|old| *old != filter) {
        return Err(operand("target_filter", "conflicting"));
    }
    target.filter = Some(filter);
    Ok(slot)
}

fn bind_object_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    selection: &str,
    domain: TargetDomain,
    optional: bool,
) -> Result<u32, Failure> {
    if (optional || program.optional_target)
        && !program.targets.is_empty()
        && !(program.optional_target == optional && slots.contains_key(selection))
    {
        return Err(operand("target", "up_to:mixed_occurrences"));
    }
    let slot = bind_target(program, slots, selection, domain)?;
    program.optional_target = optional;
    Ok(slot)
}

/// Cards of one of your zones chosen or selected at resolution.
#[derive(Debug, PartialEq)]
pub(crate) struct ChosenCards {
    pub from: kir::SelectionZone,
    /// The printed characteristics.
    pub filter: Pred,
    pub count: u32,
    /// "Up to": fewer may be chosen.
    pub up_to: bool,
    /// "At random": nobody chooses.
    pub random: bool,
}

/// CR 608.2c-d, 400.1: "[up to] N <cards> from your hand" or "... from your
/// graveyard": cards chosen as the instruction resolves, never targeted
/// (CR 115.10). `None` when `what` names no filtered card of either zone.
pub(crate) fn chosen_cards(what: &ast::ObjectRef) -> Result<Option<ChosenCards>, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let ast::ObjectRef::Filter(chosen) = what else {
        return Ok(None);
    };
    let Some(zone) = chosen.filter.zone.as_deref() else {
        return Ok(None);
    };
    let from = match zone.zone {
        ast::Zone::Hand => kir::SelectionZone::Hand,
        ast::Zone::Graveyard => kir::SelectionZone::Graveyard,
        _ => return Ok(None),
    };
    let reject = |detail: &str| operand("chosen_cards", detail);
    if zone.position.is_some()
        || zone.depth.is_some()
        || zone.owner.as_deref() != Some(&Owner::Your)
        || chosen.filter.cls != Some(ast::ObjectFilterCls::Card)
    {
        return Err(reject("zone"));
    }
    // CR 608.2d: "at random" from your graveyard is a selection nobody
    // makes, of exactly as many cards as it can.
    let random = chosen.random.is_some();
    if random && (from != kir::SelectionZone::Graveyard || chosen.up_to.is_some()) {
        return Err(reject("random"));
    }
    if chosen.group_constraints.is_some() {
        return Err(reject("group_constraints"));
    }
    let count = match &*chosen.count {
        ast::Amount::Fixed(n) => u32::try_from(n.value).ok().filter(|n| (1..=7).contains(n)),
        _ => None,
    }
    .ok_or_else(|| reject("count"))?;
    let up_to = match chosen.up_to {
        None => false,
        Some(true) => true,
        Some(false) => return Err(reject("up_to")),
    };
    let mut printed = (*chosen.filter).clone();
    printed.zone = None;
    let permanent = printed.permanent.take();
    let mut predicate = lower_card_filter(&printed)?;
    if permanent.is_some() {
        // CR 110.4: the permanent card types in the admitted catalog.
        predicate.0.push(Atom::AnyType {
            kinds: vec![
                CardKind::Artifact,
                CardKind::Creature,
                CardKind::Enchantment,
                CardKind::Land,
                CardKind::Planeswalker,
            ],
        });
    }
    Ok(Some(ChosenCards {
        from,
        filter: predicate,
        count,
        up_to,
        random,
    }))
}

/// CR 601.2c, 608.2b. Bind a graveyard card with the printed ownership
/// and characteristic restrictions.
pub(crate) fn bind_graveyard_target(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &ast::ObjectRef,
    own_only: bool,
) -> Result<u32, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let (filter, selection, optional) = target_occurrence(what)?;
    let invalid_filter = || operand("target", "own_graveyard_filter");
    let zone = filter
        .zone
        .as_deref()
        .filter(|z| z.zone == ast::Zone::Graveyard && z.position.is_none() && z.depth.is_none())
        .ok_or_else(invalid_filter)?;
    if filter.cls != Some(ast::ObjectFilterCls::Card) {
        return Err(invalid_filter());
    }
    let foreign = match zone.owner.as_deref() {
        Some(Owner::Your) => None,
        Some(Owner::Any) if !own_only => Some(TargetDomain::GraveyardCard {
            opponents_only: false,
        }),
        Some(Owner::AnOpponent) if !own_only => Some(TargetDomain::GraveyardCard {
            opponents_only: true,
        }),
        _ => return Err(invalid_filter()),
    };
    let mut printed = filter.clone();
    printed.zone = None;
    let permanent = printed.permanent.take();
    let multicolored = printed.multicolored.take();
    // CR 107.3, 202.3, 601.2b-c: the only dynamic property admitted here
    // is a card's printed mana value at most the spell's announced X.
    let at_most_x = foreign.is_none() && printed.properties.as_deref().is_some_and(|properties| {
        matches!(properties, [property] if property.property == ast::PropertyConstraintProperty::ManaValue
            && property.comparison.op == ast::ComparisonOp::Le
            && matches!(&*property.comparison.value, ast::Amount::X(_)))
    });
    if at_most_x {
        printed.properties = None;
    }
    let mut predicate = lower_card_filter(&printed).map_err(|_| invalid_filter())?;
    if at_most_x {
        predicate.0.push(Atom::ManaValue {
            cmp: Cmp::AtMost,
            value: Amount::announced_x(),
        });
    }
    if let Some(multicolored) = multicolored {
        if !multicolored {
            return Err(invalid_filter());
        }
        // CR 105.1–2: two different colors suffice. Test color, not color
        // identity (CR 903.4), using the existing card predicate evaluator.
        let colors = [Color::W, Color::U, Color::B, Color::R, Color::G];
        let alternatives = colors
            .iter()
            .enumerate()
            .flat_map(|(index, first)| {
                colors[index + 1..].iter().map(move |second| {
                    Pred(vec![
                        Atom::Color { color: *first },
                        Atom::Color { color: *second },
                    ])
                })
            })
            .collect();
        predicate.0.push(Atom::AnyOf { alternatives });
    }
    if permanent.is_some() {
        // CR 110.4: the permanent card types in the admitted catalog.
        predicate.0.push(Atom::AnyType {
            kinds: vec![
                CardKind::Artifact,
                CardKind::Creature,
                CardKind::Enchantment,
                CardKind::Land,
                CardKind::Planeswalker,
            ],
        });
    }
    let domain = if let Some(domain) = foreign {
        domain
    } else if predicate.0.contains(&Atom::Type {
        kind: CardKind::Creature,
    }) {
        TargetDomain::OwnGraveyardCreatureCard
    } else {
        TargetDomain::OwnGraveyardCard
    };
    // Preserve the existing wire shape for unqualified cards and type-only
    // alternatives. Everything else keeps its exact conjunction.
    let simple = if permanent.is_none()
        && present_fields(filter)
            .iter()
            .all(|field| matches!(*field, "cls" | "zone" | "types" | "all_of"))
    {
        match (filter.types.as_deref(), filter.all_of.as_deref()) {
            (None, None) => Some(vec![]),
            (Some([kind]), None) => Some(vec![card_kind(kind, "target_filter")?]),
            (None, Some(group)) => type_alternatives(group, "target_filter").ok(),
            _ => None,
        }
    } else {
        None
    };
    let criteria = match simple {
        Some(kinds) if kinds.is_empty() || domain == TargetDomain::OwnGraveyardCreatureCard => None,
        Some(kinds) => Some(PermanentFilter::any().with_types(kinds)),
        None => Some(PermanentFilter {
            predicate: Some(predicate),
            ..PermanentFilter::any()
        }),
    };
    let already_bound = slots.contains_key(selection);
    let slot = bind_object_target(program, slots, selection, domain, optional)?;
    let target = &mut program.targets[slot as usize];
    if already_bound && target.filter != criteria {
        return Err(operand("target_filter", "conflicting"));
    }
    target.filter = criteria;
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

/// CR 613.4c: a fixed signed power or toughness change.
pub(crate) fn signed_fixed(value: &ast::SignedAmount) -> Result<Quantity, Failure> {
    let ast::SignedAmount { sign, amount } = value;
    let ast::Amount::Fixed(n) = &**amount else {
        return Err(operand("signed_amount", variant(&**amount)));
    };
    let value = u32::try_from(n.value)
        .map(i64::from)
        .map_err(|_| operand("signed_amount", "fixed:range"))?;
    Ok(Quantity::from(match **sign {
        ast::SignedAmountSign::N1 => value,
        ast::SignedAmountSign::Minus1 => -value,
    }))
}

pub(crate) fn lower_other_casts_mana_value(
    sum: &ast::AmountCastManaValueSum,
    context: Context,
) -> Result<Amount, Failure> {
    match (&*sum.who, &*sum.excluding) {
        (ast::PlayerRef::You(_), ast::ObjectRef::This(this)) if this.noun == "spell" => {
            Ok(kir::Amount::OtherCastsManaValue {
                caster: PlayerExpr::you(context),
            })
        }
        _ => Err(operand("amount", "cast_mana_value_sum:scope")),
    }
}

pub(crate) fn fixed_product(factors: Vec<Amount>) -> Result<Amount, Failure> {
    if factors.len() < 2 {
        return Err(operand("amount", "product:arity"));
    }
    let mut factor = Quantity::from(1);
    let mut of = None;
    for value in factors {
        match value {
            Amount::Fixed { value } if !value.is_negative() => factor.multiply(&value),
            value if of.is_none() => of = Some(value),
            _ => return Err(operand("amount", "product:several_dynamic_factors")),
        }
    }
    Ok(match of {
        Some(of) if factor == Quantity::from(1) => of,
        Some(of) if factor > Quantity::zero() => Amount::Times {
            factor,
            of: Box::new(of),
        },
        _ => Amount::Fixed { value: factor },
    })
}

/// CR 400.2, 402.3, 608.2h: count objects matching a public filter, or the
/// size of your hand. The engine never counts filtered hidden cards.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn lower_count(count: &ast::AmountCount, context: Context) -> Result<Amount, Failure> {
    lower_count_with(count, context, |filter| lower_filter(filter, context))
}

pub(crate) fn lower_count_with(
    count: &ast::AmountCount,
    context: Context,
    lower: impl FnOnce(&ast::ObjectFilter) -> Result<Pred, Failure>,
) -> Result<Amount, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let mut filter = (*count.of).clone();
    let zone = match filter.zone.take().as_deref() {
        None
        | Some(ast::ZoneRef {
            zone: ast::Zone::Battlefield,
            owner: None,
            depth: None,
            position: None,
        }) => kir::ZoneExpr::Battlefield,
        Some(ast::ZoneRef {
            zone: ast::Zone::Graveyard,
            owner,
            depth: None,
            position: None,
        }) => match owner.as_deref() {
            Some(Owner::Your) => kir::ZoneExpr::Graveyard {
                of: PlayerExpr::you(context),
            },
            Some(Owner::Any | Owner::EachPlayer | Owner::AllPlayers) => kir::ZoneExpr::Graveyards,
            _ => return Err(operand("amount", "count:graveyard_owner")),
        },
        Some(ast::ZoneRef {
            zone: ast::Zone::Hand,
            owner,
            depth: None,
            position: None,
        }) if owner.as_deref() == Some(&Owner::Your) => kir::ZoneExpr::Hand {
            of: PlayerExpr::you(context),
        },
        Some(other) => return Err(operand("amount", format!("count:{}", variant(&other.zone)))),
    };
    if zone != kir::ZoneExpr::Battlefield {
        if filter.cls == Some(ast::ObjectFilterCls::Card) {
            filter.cls = None;
        }
        if matches!(zone, kir::ZoneExpr::Hand { .. }) && !present_fields(&filter).is_empty() {
            return Err(operand("amount", "count:filtered_hand"));
        }
    }
    Ok(Amount::Count {
        set: SetExpr::Matching {
            zone,
            filter: lower(&filter)?,
        },
    })
}

/// CR 608.2h: "[N] for each [filter]" is N times the number of objects the
/// filter matches as the step is performed, counted once. A filter term
/// outside the count tables, or an object reference ("for each creature
/// destroyed this way"), rejects.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn lower_for_each(
    for_each: &serde_json::Value,
    per_object: &ast::Amount,
    context: Context,
) -> Result<Amount, Failure> {
    lower_each_filter(typed(for_each, "forEach")?, per_object, context)
}

pub(crate) fn lower_each_filter(
    filter: ast::ObjectFilter,
    per_object: &ast::Amount,
    context: Context,
) -> Result<Amount, Failure> {
    let factor = match per_object {
        ast::Amount::Fixed(n) if n.value > 0 && u32::try_from(n.value).is_ok() => n.value,
        _ => return Err(operand("forEach", "per_object_amount")),
    };
    let count = lower_count(
        &ast::AmountCount {
            of: Box::new(filter),
        },
        context,
    )?;
    Ok(if factor == 1 {
        count
    } else {
        Amount::Times {
            factor: Quantity::from(factor),
            of: Box::new(count),
        }
    })
}

/// CR 107.3, 608.2h: the sentence binding "where X is [a number]" as the
/// amount each step that reads X evaluates when it is performed: a count
/// of a public set, the greatest power, toughness or mana value among
/// permanents, or your devotion (CR 700.5). A count announced at casting,
/// another player's referent or any other quantity rejects by name.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn lower_x_binding(
    bindings: &[serde_json::Value],
    context: Context,
) -> Result<Option<kir::Amount>, Failure> {
    let bindings: Vec<ast::SentenceBindingsItem> = typed(&bindings, "bindings")?;
    let [binding] = bindings.as_slice() else {
        return if bindings.is_empty() {
            Ok(None)
        } else {
            Err(operand("bindings", "several"))
        };
    };
    // The archive binds only X (`LitX`); decoding has already proved it.
    let ast::LitX::X = binding.variable;
    if binding.as_of.is_some() {
        return Err(operand("bindings", "as_of"));
    }
    match &*binding.value {
        ast::Amount::Count(count) => lower_count(count, context).map(Some),
        amount @ ast::Amount::CounterCount(_) => source_counter_amount(amount).map(Some),
        ast::Amount::AttributeExtremum(extremum) => {
            if extremum.extremum != ast::AmountAttributeExtremumExtremum::Maximum {
                return Err(operand("bindings", "attribute_extremum:minimum"));
            }
            let Amount::Count {
                set:
                    set @ SetExpr::Matching {
                        zone: kir::ZoneExpr::Battlefield,
                        ..
                    },
            } = lower_count(
                &ast::AmountCount {
                    of: extremum.among.clone(),
                },
                context,
            )?
            else {
                return Err(operand("bindings", "attribute_extremum:zone"));
            };
            Ok(Some(Amount::Greatest {
                set,
                stat: match extremum.attribute {
                    ast::ObjectFilterImplicitComparisonAttribute::Power => kir::Stat::Power,
                    ast::ObjectFilterImplicitComparisonAttribute::Toughness => kir::Stat::Toughness,
                    ast::ObjectFilterImplicitComparisonAttribute::ManaValue => kir::Stat::ManaValue,
                },
            }))
        }
        ast::Amount::Devotion(devotion) => lower_devotion(devotion, context).map(Some),
        other => Err(operand("bindings", variant(other))),
    }
}

pub(crate) fn lower_devotion(
    devotion: &ast::AmountDevotion,
    context: Context,
) -> Result<Amount, Failure> {
    if !matches!(*devotion.who, ast::PlayerRef::You(_)) {
        return Err(operand("bindings", "devotion:player"));
    }
    let colors = match &*devotion.colors {
        ast::AmountDevotionColors::Array((a,)) => vec![color(a)],
        ast::AmountDevotionColors::Array2((a, b)) => vec![color(a), color(b)],
        ast::AmountDevotionColors::Choice(_) => {
            return Err(operand("bindings", "devotion:chosen"));
        }
    };
    Ok(Amount::Devotion {
        player: PlayerExpr::you(context),
        colors,
    })
}
/// CR 109.5, 119.3: the player a step names. "You" is the controller of
/// this kind of stack object; "that player" is the trigger's event player.
pub(crate) fn lower_player_ref(
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    who: ast::PlayerRef,
    context: Context,
    event_player: Option<kir::EventPlayerRole>,
) -> Result<PlayerExpr, Failure> {
    match who {
        ast::PlayerRef::You(_) => Ok(PlayerExpr::you(context)),
        ast::PlayerRef::They(_) if event_player == Some(kir::EventPlayerRole::DrawingPlayer) => {
            Ok(PlayerExpr::event(kir::EventPlayerRole::DrawingPlayer))
        }
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
        ast::ZoneRef {
            zone: ast::Zone::Library,
            position: Some(ast::ObjectFilterZoneChangeHistoryFromZoneRefPosition::Bottom),
            depth: None,
            owner: Some(owner),
        } if matches!(
            *owner,
            ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner::ItsOwner
        ) =>
        {
            Ok(kir::Destination::OwnerLibraryBottom)
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

/// CR 201.5: these nouns identify the ability's source incarnation. They
/// impose no current type requirement when the effect is performed.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub(crate) fn is_this_permanent(what: &ast::ObjectRef) -> bool {
    matches!(what, ast::ObjectRef::This(this) if matches!(this.noun.as_str(),
        "creature" | "artifact" | "enchantment" | "land" | "permanent" | "Aura" | "aura"))
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

/// CR 122.1, 608.2h: "[N for each | the number of] [kind] counter(s) on
/// this [permanent]", read as the verb is performed. Other recipients,
/// "any" kinds and totals reject.
pub(crate) fn source_counter_amount(amount: &ast::Amount) -> Result<kir::Amount, Failure> {
    let (factor, count) = match amount {
        ast::Amount::CounterCount(count) => (1, &**count),
        ast::Amount::Product(product) => match product.factors.as_slice() {
            [ast::Amount::Fixed(n), ast::Amount::CounterCount(count)]
                if n.value > 0 && u32::try_from(n.value).is_ok() =>
            {
                (n.value, &**count)
            }
            _ => return Err(operand("amount", "product")),
        },
        other => return Err(operand("amount", variant(other))),
    };
    let ast::AmountCounterCount { counter, on, total } = count;
    let (ast::ObjectFilterCounterConstraintsItemCounter::CounterSpec(spec), None) =
        (&**counter, total)
    else {
        return Err(operand("amount", "counter_count:kind"));
    };
    let ast::AmountCounterCountOn::ObjectRef(on) = &**on else {
        return Err(operand("amount", "counter_count:player"));
    };
    if !is_this_permanent(on) {
        return Err(operand("amount", "counter_count:recipient"));
    }
    let of = kir::Amount::CountersOn {
        object: kir::Ref::Source,
        counter: object_counter_kind(&spec.r#type)?,
    };
    Ok(if factor == 1 {
        of
    } else {
        kir::Amount::Times {
            factor: Quantity::from(factor),
            of: Box::new(of),
        }
    })
}

/// CR 122.1: the permanent counter kinds whose rules the engine executes.
pub(crate) fn object_counter_kind(name: &str) -> Result<kir::CounterKind, Failure> {
    match name {
        "+1/+1" => Ok(kir::CounterKind::PlusOnePlusOne),
        "-1/-1" => Ok(kir::CounterKind::MinusOneMinusOne),
        // CR 122.1b: a keyword counter grants exactly its keyword; the
        // kernel check admits only keywords whose grant the runtime executes.
        name @ ("flying" | "first strike" | "double strike" | "deathtouch" | "haste"
        | "hexproof" | "indestructible" | "lifelink" | "menace" | "reach" | "trample"
        | "vigilance") => Ok(kir::CounterKind::Keyword {
            keyword: serde_json::from_value(serde_json::Value::String(name.into()))
                .map_err(|_| unsupported("effects.steps", "verb:put-counters:counter_kind"))?,
        }),
        "stun" => Ok(kir::CounterKind::Stun),
        "lore" => Ok(kir::CounterKind::Lore),
        name if kir::plain_counter_name(name) => Ok(kir::CounterKind::Named { name: name.into() }),
        _ => Err(unsupported(
            "effects.steps",
            "verb:put-counters:counter_kind",
        )),
    }
}
