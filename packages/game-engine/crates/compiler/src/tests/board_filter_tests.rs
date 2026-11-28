//! General board filters: every printed filter term of a destroy, exile or
//! return-all step becomes one kernel predicate atom, or the step rejects.
//! CR 205, 208.1, 202.3, 608.2h (pinned 2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::json;

/// The board set an archived `all`/`each` operand lowers to.
fn board(what: &serde_json::Value, context: Context) -> Result<SetExpr, Failure> {
    lower_board_set(&object(what)?, context)
}
fn set(filter: serde_json::Value) -> Result<Pred, Failure> {
    match board(&json!({"ref":"all","filter":filter}), Context::Spell)? {
        SetExpr::Matching { filter, .. } => Ok(filter),
        other => panic!("{other:?}"),
    }
}
fn kind(kind: CardKind) -> Atom {
    Atom::Type { kind }
}

#[test]
fn closed_legacy_filters_lower_to_the_same_sets_as_before() {
    for (filter, legacy) in [
        (json!({"types":["creature"]}), ObjectSet::Creatures),
        (
            json!({"nonTypes":["land"],"cls":"permanent"}),
            ObjectSet::NonlandPermanents,
        ),
        (json!({"types":["enchantment"]}), ObjectSet::Enchantments),
        (
            json!({"types":["creature"],"status":["attacking"]}),
            ObjectSet::AttackingCreatures,
        ),
        (
            json!({"cls":"permanent","control":"you"}),
            ObjectSet::ControllerPermanents,
        ),
        (
            json!({"types":["creature"],"control":"you"}),
            ObjectSet::ControllerCreatures,
        ),
        (
            json!({"types":["creature"],"control":"opponent"}),
            ObjectSet::OpponentCreatures,
        ),
        (
            json!({"types":["creature"],"withAbility":["flying"]}),
            ObjectSet::FlyingCreatures,
        ),
        (
            json!({"types":["creature"],"withoutAbility":["flying"]}),
            ObjectSet::NonflyingCreatures,
        ),
        (
            json!({"nonTypes":["land"],"cls":"permanent","control":"opponent"}),
            ObjectSet::OpponentNonlandPermanents,
        ),
    ] {
        assert_eq!(
            object(&json!({"ref":"all","filter":filter}))
                .and_then(|what| lower_reviewed_set(&what, Context::Spell))
                .ok(),
            {
                // The reviewed sets never named the opponents' nonland permanents.
                (legacy != ObjectSet::OpponentNonlandPermanents)
                    .then(|| legacy.kernel(Context::Spell))
            }
        );
        assert_eq!(
            board(&json!({"ref":"each","filter":filter}), Context::Spell).unwrap(),
            legacy.kernel(Context::Spell),
            "{legacy:?}"
        );
    }
}

#[test]
fn general_filter_terms_each_become_one_atom_in_fixed_order() {
    let cases = [
        (
            json!({"types":["planeswalker"]}),
            vec![kind(CardKind::Planeswalker)],
        ),
        // "Destroy all artifacts and enchantments."
        (
            json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}]}),
            vec![Atom::AnyType {
                kinds: vec![CardKind::Artifact, CardKind::Enchantment],
            }],
        ),
        // "Destroy all nonbasic lands."
        (
            json!({"types":["land"],"nonSupertypes":["basic"]}),
            vec![kind(CardKind::Land), Atom::NotBasic],
        ),
        // "Destroy all Islands."
        (
            json!({"subtypes":["Island"]}),
            vec![Atom::Subtype {
                subtype: "Island".into(),
            }],
        ),
        // "Destroy all non-Aura enchantments."
        (
            json!({"types":["enchantment"],"nonSubtypes":["Aura"]}),
            vec![
                kind(CardKind::Enchantment),
                Atom::NotSubtype {
                    subtype: "Aura".into(),
                },
            ],
        ),
        // "Return all nonblue creatures to their owners' hands."
        (
            json!({"types":["creature"],"nonColors":["blue"]}),
            vec![kind(CardKind::Creature), Atom::NotColor { color: Color::U }],
        ),
        // "Return all green permanents to their owners' hands."
        (
            json!({"cls":"permanent","colors":["green"]}),
            vec![Atom::Color { color: Color::G }],
        ),
        // "Destroy all tapped creatures."
        (
            json!({"types":["creature"],"status":["tapped"]}),
            vec![kind(CardKind::Creature), Atom::Tapped],
        ),
        // "Return all creature tokens" / "all nontoken creatures".
        (
            json!({"cls":"token","types":["creature"]}),
            vec![Atom::Token, kind(CardKind::Creature)],
        ),
        (
            json!({"types":["creature"],"nonToken":true}),
            vec![kind(CardKind::Creature), Atom::NotToken],
        ),
        // "Destroy all nonland permanents with mana value 1 or less."
        (
            json!({"cls":"permanent","nonTypes":["land"],"properties":[
                {"property":"mana-value","comparison":{"op":"le","value":{"amount":"fixed","value":1}}}]}),
            vec![
                Atom::NotType {
                    kind: CardKind::Land,
                },
                Atom::ManaValue {
                    cmp: Cmp::AtMost,
                    value: Amount::fixed(1),
                },
            ],
        ),
        // "Destroy all creatures with toughness 4 or greater."
        (
            json!({"types":["creature"],"properties":[
                {"property":"toughness","comparison":{"op":"ge","value":{"amount":"fixed","value":4}}}]}),
            vec![
                kind(CardKind::Creature),
                Atom::Toughness {
                    cmp: Cmp::AtLeast,
                    value: Amount::fixed(4),
                },
            ],
        ),
        // CR 107.1: strict integer comparisons close onto the next integer.
        (
            json!({"types":["creature"],"properties":[
                {"property":"power","comparison":{"op":"gt","value":{"amount":"fixed","value":3}}},
                {"property":"power","comparison":{"op":"lt","value":{"amount":"fixed","value":7}}}]}),
            vec![
                kind(CardKind::Creature),
                Atom::Power {
                    cmp: Cmp::AtLeast,
                    value: Amount::fixed(4),
                },
                Atom::Power {
                    cmp: Cmp::AtMost,
                    value: Amount::fixed(6),
                },
            ],
        ),
        // "Return all permanents to their owners' hands."
        (json!({"cls":"permanent"}), vec![]),
    ];
    for (filter, atoms) in cases {
        assert_eq!(set(filter.clone()).ok(), Some(Pred(atoms)), "{filter}");
    }
    // "You control" names the controller of this kind of stack object.
    let yours = json!({"ref":"all","filter":{"types":["artifact"],"control":"you"}});
    for context in [Context::Spell, Context::Activated, Context::Triggered] {
        assert_eq!(
            board(&yours, context).unwrap(),
            SetExpr::battlefield(Pred(vec![
                kind(CardKind::Artifact),
                Atom::ControlledBy {
                    player: PlayerExpr::you(context)
                }
            ]))
        );
    }
}

#[test]
fn unrepresented_filter_terms_reject_the_whole_set() {
    for filter in [
        // Terms with no reviewed atom.
        json!({"cls":"permanent","multicolored":true}),
        json!({"types":["creature"],"counterConstraints":[]}),
        json!({"types":["creature"],"damageHistory":{"role":"source"}}),
        json!({"types":["creature"],"other":true}),
        json!({"types":["enchantment"],"controlledBy":{"player":"target-player"}}),
        json!({"cls":"card","zone":{"owner":"your","zone":"library"}}),
        // A card type the engine does not model.
        json!({"types":["battle"]}),
        // A computed power bound, an unknown property or operator.
        json!({"types":["creature"],"properties":[
            {"property":"power","comparison":{"op":"le","value":{"amount":"x"}}}]}),
        json!({"types":["creature"],"properties":[
            {"property":"loyalty","comparison":{"op":"le","value":{"amount":"fixed","value":1}}}]}),
        json!({"types":["creature"],"properties":[
            {"property":"power","comparison":{"op":"ne","value":{"amount":"fixed","value":1}}}]}),
        json!({"types":["creature"],"properties":[
            {"property":"power","extra":1,"comparison":{"op":"le","value":{"amount":"fixed","value":1}}}]}),
        // Unknown values of known terms.
        json!({"types":["creature"],"control":"that-player"}),
        json!({"types":["creature"],"status":["blocking"]}),
        json!({"types":["creature"],"withAbility":["trample"]}),
        json!({"types":["land"],"nonSupertypes":["snow"]}),
        json!({"types":["creature"],"colors":["purple"]}),
        json!({"types":["creature"],"nonToken":false}),
        json!({"types":[]}),
        json!({"allOf":[{"anyOf":[{"types":["artifact"]}]}]}),
        json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"subtypes":["Aura"]}]}]}),
        json!("creature"),
    ] {
        assert!(set(filter.clone()).is_err(), "{filter}");
    }
    // Only an unqualified all/each reference is a board-wide set.
    for what in [
        json!({"ref":"target","filter":{"types":["creature"]}}),
        json!({"ref":"all","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}),
    ] {
        assert!(board(&what, Context::Spell).is_err(), "{what}");
    }
}
