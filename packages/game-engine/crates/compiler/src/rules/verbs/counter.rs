//! CR 115.1, 202.3, 601.2c, 701.6: counter one announced stack object.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn lower(cx: &mut StepCx<'_>, what: &CounterTarget) -> Result<Instruction, Failure> {
    uncounted(cx)?;
    if what.reference != "target" || !fixed_one(&what.count) {
        return Err(step("counter", "target occurrence"));
    }
    let (domain, filter) = match &what.filter {
        CounterFilter::Spell {
            all_of,
            types,
            non_types,
            properties,
        } => {
            let types = match (all_of, types) {
                (None, None) => vec![],
                (None, Some(types)) if types.len() == 1 => {
                    vec![card_kind(&types[0], "counter")?]
                }
                (Some(group), None) => type_alternatives(group, "counter")?,
                _ => return Err(step("counter", "spell type conjunction")),
            };
            let non_types = listed(non_types, "nonTypes")?
                .iter()
                .map(|kind| card_kind(kind, "counter"))
                .collect::<Result<Vec<_>, _>>()?;
            let predicate = match properties.as_deref() {
                None => None,
                Some([property])
                    if property.property == ast::PropertyConstraintProperty::ManaValue =>
                {
                    Some(Pred(vec![lower_fixed_property(property)?]))
                }
                _ => return Err(step("counter", "spell property")),
            };
            let filter = (!types.is_empty() || !non_types.is_empty() || predicate.is_some())
                .then_some(PermanentFilter {
                    types,
                    non_types,
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                    power: None,
                    predicate,
                });
            (TargetDomain::StackSpell, filter)
        }
        CounterFilter::Ability { any_of }
            if any_of.len() == 2
                && any_of.iter().any(|kind| kind.ability_kind == "activated")
                && any_of.iter().any(|kind| kind.ability_kind == "triggered") =>
        {
            (TargetDomain::StackActivatedOrTriggeredAbility, None)
        }
        _ => return Err(step("counter", "ability kind")),
    };
    let was_bound = cx.slots.contains_key(&what.selection_id);
    let slot = bind_target(cx.program, cx.slots, &what.selection_id, domain)?;
    let target = &mut cx.program.targets[slot as usize];
    if was_bound && target.filter != filter {
        return Err(step("counter", "conflicting spell filter"));
    }
    target.filter = filter;
    Ok(cx.act(Verb::Counter {
        what: kir::Ref::Target { slot },
    }))
}
