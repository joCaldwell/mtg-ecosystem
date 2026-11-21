use super::*;
use crate::engine::equipped_indestructible_tests::{
    answer, both, object, prepared, recover, request,
};

#[test]
fn modal_land_picks_the_face_before_entry_and_keeps_it_through_replay() {
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json");
    const ROW: &str = include_str!("../../../../tests/fixtures/phase1-r3/branchloft-pathway.json");
    let compiled = mtg_compiler::compile(HEADER, ROW);
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    let mut p = prepared(&[("Forest", true)], false);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, budget| {
        let player = PlayerId(0);
        let other = PlayerId(1);
        let id = object(g, 0);
        let observe = |game: &Game, viewer| {
            let QueryResult::Complete(QueryValue::Observation(observation)) =
                game.observe(viewer, Budget::unlimited())
            else {
                panic!("expected observation")
            };
            observation
        };
        let own = observe(g, player);
        let row = own
            .view
            .objects
            .iter()
            .find(|row| row.handle == g.handle_for(player, id).unwrap())
            .unwrap();
        assert_eq!(row.display_face.as_deref(), Some("Branchloft Pathway"));
        assert!(
            observe(g, other)
                .view
                .objects
                .iter()
                .all(|row| row.display_face.is_none())
        );
        let beginnings = g.player_session(player).action_beginnings(
            own.decision.as_ref().unwrap().token().clone(),
            ActionScope::Object {
                object: row.handle.clone(),
            },
            64,
            Budget::unlimited(),
        );
        let ActionQueryOutcome::Complete { page } = beginnings.outcome() else {
            panic!("expected action choices")
        };
        assert!(
            page.beginnings
                .iter()
                .any(|row| matches!(row.initiation, ActionInitiation::PlayBackLand { .. }))
        );

        let mut front = g.fork();
        answer(&mut front, budget, Response::PlayLand { card: id.card });
        request(&mut front, budget);
        let front_id = object(&front, 0);
        answer(
            &mut front,
            budget,
            Response::ActivateMana { object: front_id },
        );
        request(&mut front, budget);
        assert_eq!(
            front.state.rules.players[0].mana.get(&Color::G),
            Some(&1.into())
        );
        recover(&front);

        answer(g, budget, Response::PlayBackLand { card: id.card });
        request(g, budget);
        let back = object(g, 0);
        assert_eq!(back.incarnation, id.incarnation + 1);
        assert_eq!(
            g.copiable_definition(back).unwrap().name,
            "Boulderloft Pathway"
        );
        assert_eq!(
            g.definition(id.card).unwrap().source_face_name(),
            "Branchloft Pathway"
        );
        assert!(
            observe(g, other)
                .view
                .objects
                .iter()
                .any(|row| row.display_face.as_deref() == Some("Boulderloft Pathway"))
        );
        answer(g, budget, Response::ActivateMana { object: back });
        request(g, budget);
        assert_eq!(
            g.state.rules.players[0].mana.get(&Color::W),
            Some(&1.into())
        );
        assert_eq!(g.state.rules.players[0].mana.get(&Color::G), None);
    });
}

#[test]
fn modal_land_spell_front_cancels_then_plays_the_tapped_land_back() {
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json");
    const ROW: &str = include_str!("../../../../tests/fixtures/phase1-r3/bala-ged-recovery.json");
    let compiled = mtg_compiler::compile(HEADER, ROW);
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    let mut p = prepared(
        &[("Forest", true), ("Forest", true), ("Forest", false)],
        false,
    );
    let player = PlayerId(0);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    p.config.cards[1].zone = Zone::Graveyard { owner: player };
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, budget| {
        let original = object(g, 0);
        answer(
            g,
            budget,
            Response::Cast {
                card: original.card,
            },
        );
        assert!(matches!(request(g, budget), Request::Targets { .. }));
        let now = object(g, 0);
        assert_ne!(now, original);
        assert_eq!(
            g.copiable_definition(now).unwrap().characteristics.kind,
            CardKind::Sorcery
        );
        let target = object(g, 1);
        answer(
            g,
            budget,
            Response::Targets {
                objects: vec![target],
            },
        );
        answer(g, budget, Response::FinishMana { undo_mana: true });
        request(g, budget);
        assert_eq!(g.object_id(original.card), Some(original));
        assert_eq!(g.zone(original), Some(Zone::Hand { owner: player }));
        assert!(g.state.rules.stack.is_empty());
        recover(g);

        let frontier = g.state_digest().unwrap();
        let priority = request(g, budget);
        assert!(
            g.respond(
                player,
                priority.decision_id(),
                Response::PlayLand {
                    card: original.card
                }
            )
            .is_err()
        );
        assert_eq!(g.state_digest().unwrap(), frontier);
        answer(
            g,
            budget,
            Response::PlayBackLand {
                card: original.card,
            },
        );
        request(g, budget);
        let entered = g.current(object(g, 0)).unwrap();
        assert!(entered.tapped);
        assert_eq!(
            g.copiable_definition(entered.id)
                .unwrap()
                .characteristics
                .kind,
            CardKind::Land
        );
        assert_eq!(
            g.copiable_definition(entered.id).unwrap().name,
            "Bala Ged Sanctuary"
        );
    });
}

#[test]
fn transform_paid_original_incarnation_only_transforms_once() {
    const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json");
    const ROW: &str = include_str!("../../../../tests/fixtures/phase1-r3/thraben-gargoyle.json");
    let compiled = mtg_compiler::compile(HEADER, ROW);
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    let mut p = prepared(&[("Forest", false); 13], false);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();

    both(p, |g, budget| {
        let source = object(g, 0);
        let original = g.current(source).unwrap().clone();
        for first in [1, 7] {
            answer(
                g,
                budget,
                Response::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            );
            for i in first..first + 6 {
                let land = object(g, i);
                answer(g, budget, Response::ActivateMana { object: land });
            }
            answer(g, budget, Response::FinishMana { undo_mana: false });
            if matches!(request(g, budget), Request::SpendMana { .. }) {
                answer(
                    g,
                    budget,
                    Response::SpendMana {
                        spending: BTreeMap::from([(Color::G, 6.into())]),
                    },
                );
            }
            request(g, budget);
        }
        assert_eq!(g.state.rules.stack.len(), 2);
        assert!(g.has_keyword(source, KeywordAbility::Defender));
        recover(g);

        while g.state.rules.stack.len() == 2 {
            answer(g, budget, Response::Pass);
            request(g, budget);
        }
        assert_eq!(g.state.rules.stack.len(), 1);
        assert_eq!(g.current(source).unwrap(), &original);
        assert!(g.has_keyword(source, KeywordAbility::Flying));
        assert!(!g.has_keyword(source, KeywordAbility::Defender));
        assert_eq!(
            g.copiable_definition(source).unwrap().name,
            "Stonewing Antagonizer"
        );
        assert_eq!(
            g.derived_characteristics(source).unwrap().power,
            Some(4.into())
        );
        assert_eq!(g.physical_transform_mana_value(source), Some(1.into()));
        recover(g);

        while !g.state.rules.stack.is_empty() {
            answer(g, budget, Response::Pass);
            request(g, budget);
        }
        assert_eq!(g.current(source).unwrap(), &original);
        assert!(g.has_keyword(source, KeywordAbility::Flying));
        assert!(g.back_face_sequence(source).is_some());
        assert_eq!(g.state.rules.back_face_objects.len(), 1);
    });
}
