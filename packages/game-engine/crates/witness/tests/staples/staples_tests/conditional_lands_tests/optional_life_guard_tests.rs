//! Explicit synthetic guard probes: no optional-entry runtime support claim.
//! The private legal100 catalog/config is prepared before Game construction.
use super::*;
const SHOCK: &str = include_str!(
    "../../../../../../tests/fixtures/ramos/20283c4a-f1f0-42f0-bc08-6da87474426b.json"
);
fn optional_entry_game(library: bool) -> Game {
    let mut p = midgame(&[("Cinder Glade", 0, 0, true), ("Forest", 0, 0, false)]);
    let shock = lower(SHOCK);
    let old = lower(CINDER).oracle_id;
    let mut catalog = (*p.catalog).clone();
    for card in &mut p.config.cards {
        if card.oracle_id == old {
            card.oracle_id = shock.oracle_id.clone();
            if library {
                card.zone = Zone::Library { owner: card.owner };
            }
        }
    }
    catalog.definitions.push(shock);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(&catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p.catalog = Arc::new(catalog);
    Game::new(p)
}
#[test]
fn optional_life_lands_unprepared_battlefield_moves_reject_without_mutation() {
    for library in [false, true] {
        let mut g = optional_entry_game(library);
        let _ = request(&mut g);
        let shock = named(&g, "Breeding Pool");
        let basic = named(&g, "Forest");
        // A valid earlier movement must roll back when the optional entry fails.
        let before = g.state.clone();
        let trace = g.trace.clone();
        let error = g
            .commit_batch(
                BatchCause::Fixture,
                EventGroup::Ordered {
                    children: vec![
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::Move {
                                object: basic,
                                destination: Zone::Graveyard { owner: PlayerId(0) },
                            },
                        },
                        EventGroup::Occurrence {
                            proposed: ProposedEvent::Move {
                                object: shock,
                                destination: Zone::Battlefield,
                            },
                        },
                    ],
                },
            )
            .unwrap_err();
        assert!(error.contains("prepared transaction authority"), "{error}");
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
    }
}
#[test]
fn optional_life_lands_forced_search_entry_rejects_without_implicit_decline() {
    // Synthetic changed entry clause on the real paid Farseek producer. Its
    // source identities/admission are not claimed as a frozen shock witness.
    let mut p = natural_prepared(FARSEEK);
    let catalog = Arc::make_mut(&mut p.catalog);
    let d = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Cinder Glade")
        .unwrap();
    d.static_abilities[0].effect = StaticEffect::EntersTappedUnlessPayLife { amount: 2.into() };
    d.source_labels = None;
    d.capabilities = d.requirements();
    catalog.validate().unwrap();
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    let mut g = Game::new(p);
    let _ = natural_search(&mut g, "Farseek", 2);
    let shock = named(&g, "Cinder Glade");
    assert!(g.searched_land_force_tapped(PlayerId(0)).unwrap());
    assert!(g.search_lands(PlayerId(0)).contains(&shock));
    let before = g.state.clone();
    let trace = g.trace.clone();
    let error = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::SearchLandEntry {
                    object: shock,
                    player: PlayerId(0),
                },
            },
        )
        .unwrap_err();
    assert!(error.contains("prepared transaction authority"), "{error}");
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}
#[test]
fn optional_life_lands_nonbattlefield_movement_stays_outside_entry_preparation() {
    for destination in [
        Zone::Hand { owner: PlayerId(0) },
        Zone::Graveyard { owner: PlayerId(0) },
    ] {
        let mut g = optional_entry_game(true);
        let _ = request(&mut g);
        let shock = named(&g, "Breeding Pool");
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Move {
                    object: shock,
                    destination,
                },
            },
        )
        .unwrap();
        assert_eq!(g.zone(g.object_id(shock.card).unwrap()), Some(destination));
    }
}
