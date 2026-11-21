use super::*;

#[test]
fn granted_land_mana_pays_cast_and_rechecks_departed_grant() {
    use mtg_witness::equipped::{answer, recover, request};
    let manifest =
        include_str!("../../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let source = include_str!(
        "../../../../../tests/fixtures/ramos/539f5396-d99a-417d-a84c-dff7930b5900.json"
    );
    let compiled = mtg_compiler::compile(manifest, source);
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    let oracle = definition.oracle_id.clone();
    let mut p = Prepared::admit(
        Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-ir.json"
        ))
        .unwrap(),
        ScenarioConfig::land_return(),
    )
    .unwrap();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let caster = PlayerId(0);
    let add = |name: &str, owner, controller, zone| {
        let row = catalog
            .definitions
            .iter()
            .find(|d| d.name == name || d.oracle_id == name)
            .unwrap();
        CardSetup {
            id: CardId(0),
            oracle_id: row.oracle_id.clone(),
            owner,
            controller,
            zone,
        }
    };
    p.config.cards = vec![
        add(&oracle, PlayerId(1), caster, Zone::Battlefield),
        add("Forest", caster, caster, Zone::Battlefield),
        add("Unsummon", caster, caster, Zone::Hand { owner: caster }),
        add("Grizzly Bears", PlayerId(2), PlayerId(2), Zone::Battlefield),
    ];
    for (id, card) in p.config.cards.iter_mut().enumerate() {
        card.id = CardId(id as u32);
    }
    p.profile_digest = digest(&p.config).unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    let mut game = Game::with_trace(p, TraceMode::Detailed);
    let land = game.object_id(CardId(1)).unwrap();
    let target = game.object_id(CardId(3)).unwrap();
    let advertised = game
        .mana_abilities(land)
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(advertised, BTreeSet::from(Game::granted_land_mana_colors()));

    answer(&mut game, 1, Response::Cast { card: CardId(2) });
    answer(
        &mut game,
        1,
        Response::Targets {
            objects: vec![target],
        },
    );
    answer(&mut game, 1, Response::ActivateMana { object: land });
    let Request::ChooseManaAbility { choices, .. } = request(&mut game, 1) else {
        panic!("missing land ability choice");
    };
    assert!(choices.contains(&ManaAbilityIdentity::Intrinsic { color: Color::G }));
    assert!(choices.contains(&ManaAbilityIdentity::GrantedLandTap { color: Color::U }));
    assert!(!choices.contains(&ManaAbilityIdentity::GrantedLandTap { color: Color::C }));
    let mut departed = game.fork();
    departed.concede(PlayerId(1)).unwrap();
    assert!(matches!(request(&mut departed, 1), Request::Mana { .. }));
    assert!(!departed.current(land).unwrap().tapped);
    assert!(!departed.has_granted_land_mana(land).unwrap());
    recover(&departed);
    // The independent intrinsic ability survives the granting artifact.
    let mut accepted = game.fork();
    let offer = request(&mut accepted, 1);
    accepted
        .respond(
            offer.player(),
            offer.decision_id(),
            Response::ChooseManaAbility {
                ability: ManaAbilityIdentity::Intrinsic { color: Color::G },
            },
        )
        .unwrap();
    accepted.concede(PlayerId(1)).unwrap();
    assert!(matches!(request(&mut accepted, 1), Request::Mana { .. }));
    assert!(accepted.current(land).unwrap().tapped);
    assert_eq!(accepted.mana(caster, Color::G), Some(1.into()));
    recover(&accepted);

    answer(
        &mut game,
        1,
        Response::ChooseManaAbility {
            ability: ManaAbilityIdentity::GrantedLandTap { color: Color::U },
        },
    );
    answer(&mut game, 1, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert!(game.top().unwrap().paid.is_some());
    assert!(game.current(land).unwrap().tapped);
    recover(&game);
}
