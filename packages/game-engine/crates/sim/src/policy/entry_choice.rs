//! Rank prospective entry choices from the chooser's deck and permitted board.
use super::definition;
use mtg_contracts::{
    CardKind, Catalog, Characteristics, Color, Definition,
    kir::{Atom, Change, Scope, StaticEffect},
};
use mtg_core::{Candidate, Decision, PermittedView, Reason, Slot, Viewer, Zone, query};
use std::cmp::Reverse;

/// CR 607.2d, 613.4c: only the linked choice feeds this fixed stat reduction.
/// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
fn harmful_filter<'a>(source: &'a Definition, choice: &Atom) -> Option<&'a [Atom]> {
    source.static_abilities.iter().find_map(|ability| {
        let StaticEffect::Continuous {
            affects: Scope::Matching { filter },
            changes,
            condition: None,
        } = &ability.effect
        else {
            return None;
        };
        let [Change::PowerToughness { power, toughness }] = changes.as_slice() else {
            return None;
        };
        let (power, toughness) = (power.as_fixed()?, toughness.as_fixed()?);
        (power <= &0.into()
            && toughness <= &0.into()
            && (power.is_negative() || toughness.is_negative())
            && filter.0.contains(choice))
        .then_some(filter.0.as_slice())
    })
}

pub(super) fn pick<'a>(
    catalog: &Catalog,
    view: &PermittedView,
    decision: &Decision<Viewer>,
    slot: &Slot,
    own_deck: impl Iterator<Item = &'a str>,
) -> Option<u32> {
    let allowed = || {
        slot.allowed
            .iter()
            .copied()
            .filter_map(|index| Some((index, decision.candidate(index).ok()?)))
    };
    let choice = &match allowed().next()?.1 {
        Candidate::Color { .. } => Atom::SourceChosenColor {},
        Candidate::CreatureType { .. } => Atom::SourceChosenCreatureType {},
        _ => return None,
    };
    let Reason::ReplacementOrder { subject } = &decision.reason else {
        return None;
    };
    let source = view
        .objects
        .iter()
        .find(|o| Some(&o.handle) == subject.as_ref());
    let filter = source.and_then(|object| {
        let printed = definition(catalog, &object.oracle_id).ok()?;
        let printed = match object.faces.as_ref().and_then(|f| f.active_face) {
            Some(face) => printed.face(face)?,
            None => printed,
        };
        harmful_filter(printed, choice)
    });
    let opponents: Vec<_> = filter
        .into_iter()
        .flat_map(|filter| {
            view.objects.iter().filter_map(move |o| {
                if o.zone != Zone::Battlefield || o.phased_out || o.controller == view.viewer {
                    return None;
                }
                let facts = query::in_view(view, o);
                let affected = filter
                    .iter()
                    .filter(|&atom| atom != choice)
                    .all(|atom| query::matches_atom(&facts, view.viewer, atom).is_yes());
                o.characteristics.as_ref().filter(|_| affected)
            })
        })
        .collect();
    let deck: Vec<_> = own_deck
        .filter_map(|id| definition(catalog, id).ok())
        .map(Definition::card_characteristics)
        .collect();
    let rank = |hits, own: usize| {
        // A missing source or unclassified effect defaults to the deck's
        // most common value. For a known shrink, prioritize opposing hits
        // and then the least represented value in the chooser's deck.
        (
            hits,
            if filter.is_some() {
                usize::MAX - own
            } else {
                own
            },
        )
    };
    match choice {
        Atom::SourceChosenCreatureType {} => allowed()
            .filter_map(|(index, candidate)| {
                let Candidate::CreatureType { kind } = candidate else {
                    return None;
                };
                let name = kind.name();
                let matches =
                    |c: &Characteristics| c.has_type(CardKind::Creature) && c.has_subtype(name);
                let hits = opponents.iter().filter(|c| matches(c)).count();
                let own = deck.iter().filter(|c| matches(c)).count();
                Some((rank(hits, own), Reverse(name), index))
            })
            .max()
            .map(|(_, _, index)| index),
        Atom::SourceChosenColor {} => {
            let commander_colors = view
                .commanders
                .iter()
                .filter(|c| c.history.owner == view.viewer)
                .flat_map(|c| c.history.color_identity.iter().copied());
            let identity = Characteristics::devotion_color_mask(commander_colors);
            allowed()
                .filter_map(|(index, candidate)| {
                    let Candidate::Color { color } = candidate else {
                        return None;
                    };
                    if *color == Color::C {
                        return None;
                    }
                    let mask = Characteristics::devotion_color_mask([*color]);
                    let hits = opponents
                        .iter()
                        .filter(|c| c.colors.contains(color))
                        .count();
                    let pips: usize = deck.iter().map(|c| c.devotion_symbols(&[*color])).sum();
                    // Commander's identity breaks pip ties, including when a
                    // view-only caller has no deck roster.
                    let in_identity = identity & mask != 0;
                    let prefer_identity = in_identity != filter.is_some();
                    Some((rank(hits, pips), prefer_identity, Reverse(*color), index))
                })
                .max()
                .map(|(_, _, _, index)| index)
        }
        _ => unreachable!("only linked color and creature-type choices are ranked"),
    }
}
