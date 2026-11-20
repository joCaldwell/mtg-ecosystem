//! CR 106.7, 106.1a and 305.6: potential output includes every land
//! ability's colored output, without paying its costs.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn opponent_land_mana_colors(&self, player: PlayerId) -> Result<Vec<Color>, String> {
        if !self.is_live(player) {
            return Err("opponent-land mana requires a live activator".into());
        }
        let mut output: [BTreeSet<Color>; 4] = std::array::from_fn(|_| BTreeSet::new());
        let mut has_dynamic_land = [false; 4];
        for object in self.state.rules.objects.values() {
            // Event and departure boundaries reconcile layer-2 control before
            // payment or queries. Frozen losing owners are virtually absent.
            if object.zone != Zone::Battlefield
                || !self.is_live(object.controller)
                || !self.is_live(self.state.rules.cards[&object.id.card].owner)
            {
                continue;
            }
            let definition = self.definition(object.id.card)?;
            let mut characteristics = definition.characteristics.clone();
            self.apply_land_subtype_grants(object.id, &mut characteristics)?;
            if !characteristics.has_type(CardKind::Land) {
                continue;
            }
            let seat = usize::from(object.controller.0);
            output[seat].extend(
                characteristics
                    .subtypes
                    .iter()
                    .filter_map(|subtype| basic_land_color(subtype)),
            );
            if self.has_granted_land_mana(object.id)? {
                output[seat].extend(Self::granted_land_mana_colors());
            }
            for ability in &definition.mana_abilities {
                if ability.production == ManaProduction::OpponentLandColors {
                    has_dynamic_land[seat] = true;
                } else {
                    output[seat].extend(
                        self.mana_colors(object.controller, &ability.production)?
                            .into_iter()
                            .filter(|color| *color != Color::C),
                    );
                }
            }
        }

        // Each dynamic land sees precisely the output sets of the other seats.
        // Start with only grounded output. Each changing pass adds at least
        // one of the twenty seat/color pairs, so empty cycles stay empty.
        loop {
            let mut next = output.clone();
            for (seat, colors) in next.iter_mut().enumerate() {
                if has_dynamic_land[seat] {
                    colors.extend(opponent_colors(&output, seat));
                }
            }
            if next == output {
                return Ok(opponent_colors(&output, usize::from(player.0))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect());
            }
            output = next;
        }
    }
}

fn opponent_colors(output: &[BTreeSet<Color>; 4], seat: usize) -> impl Iterator<Item = Color> + '_ {
    output
        .iter()
        .enumerate()
        .filter(move |(controller, _)| *controller != seat)
        .flat_map(|(_, colors)| colors.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    const ORCHARD: &str =
        include_str!("../../../../tests/fixtures/ramos/27b047e3-0d41-45e2-98e9-9391d7923a1e.json");
    const STONE: &str =
        include_str!("../../../../tests/fixtures/ramos/95560508-7ac9-4be9-8a3f-3c7d5b52807b.json");
    const LORE: &str =
        include_str!("../../../../tests/fixtures/ramos/78826359-fe63-44ad-adc4-a17ffcd710e4.json");
    const RAMOS: &str =
        include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");

    fn lower(raw: &str) -> Definition {
        match mtg_compiler::compile(MANIFEST, raw) {
            mtg_compiler::CompileResult::Lowered { definition } => *definition,
            other => panic!("{other:?}"),
        }
    }

    fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
        let mut catalog = Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-mana-ir.json"
        ))
        .unwrap();
        let ramos = lower(RAMOS);
        let commander = ramos.oracle_id.clone();
        catalog
            .definitions
            .extend([lower(ORCHARD), lower(STONE), lower(LORE), ramos]);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();

        let mut cards: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(index, (name, seat, hand))| {
                let owner = PlayerId(*seat);
                CardSetup {
                    id: CardId(index as u32),
                    oracle_id: catalog
                        .definitions
                        .iter()
                        .find(|definition| definition.name == *name)
                        .unwrap()
                        .oracle_id
                        .clone(),
                    owner,
                    controller: owner,
                    zone: if *hand {
                        Zone::Hand { owner }
                    } else {
                        Zone::Battlefield
                    },
                }
            })
            .collect();
        let mut commanders = vec![];
        for seat in 0..4 {
            let owner = PlayerId(seat);
            commanders.push(CardId(cards.len() as u32));
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: commander.clone(),
                owner,
                controller: owner,
                zone: Zone::Command { owner },
            });
            let count = cards.iter().filter(|card| card.owner == owner).count();
            for _ in count..100 {
                cards.push(CardSetup {
                    id: CardId(cards.len() as u32),
                    oracle_id: FOREST.into(),
                    owner,
                    controller: owner,
                    zone: Zone::Library { owner },
                });
            }
        }
        let config = ScenarioConfig {
            initial: ScenarioInitial {
                turn_step: TurnStep::PrecombatMain,
                ..Default::default()
            },
            mode: Mode::CommanderMatch,
            players: 4,
            profile: "opponent-land-mana-private-witness".into(),
            commanders,
            cards,
        };
        Prepared {
            catalog_digest: digest(&catalog).unwrap(),
            profile_digest: digest(&config).unwrap(),
            catalog: Arc::new(catalog),
            config,
            match_config: None,
        }
    }

    fn request(game: &mut Game) -> Request {
        match game.advance(Budget::unlimited()) {
            Advance::AwaitingDecision(request) => request,
            other => panic!("{other:?}"),
        }
    }

    fn answer(game: &mut Game, response: Response) -> Request {
        let next = request(game);
        game.respond(next.player(), next.decision_id(), response)
            .unwrap();
        request(game)
    }

    fn pass_to_resolution(game: &mut Game) -> Request {
        let mut next = request(game);
        for _ in 0..4 {
            assert!(matches!(next, Request::Priority { .. }));
            next = answer(game, Response::Pass);
        }
        next
    }

    #[test]
    fn opponent_land_mana_paid_stone_and_orchard_fund_spell() {
        let mut game = Game::with_trace(
            prepared(&[
                ("Exotic Orchard", 0, true),
                ("Fellwar Stone", 0, true),
                ("Grizzly Bears", 0, true),
                ("Sol Ring", 0, false),
                ("Forest", 1, false),
            ]),
            TraceMode::Detailed,
        );
        let player = PlayerId(0);
        answer(&mut game, Response::PlayLand { card: CardId(0) });
        let orchard = game.object_id(CardId(0)).unwrap();
        assert_eq!(game.zone(orchard), Some(Zone::Battlefield));
        assert!(matches!(
            answer(&mut game, Response::Cast { card: CardId(1) }),
            Request::Mana { .. }
        ));
        let ring = game.object_id(CardId(3)).unwrap();
        answer(&mut game, Response::ActivateMana { object: ring });
        assert!(matches!(
            answer(&mut game, Response::FinishMana { undo_mana: true }),
            Request::SpendMana { .. }
        ));
        answer(
            &mut game,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::C, 2.into())]),
            },
        );
        assert_eq!(
            game.top().unwrap().paid.as_ref(),
            Some(&game.top().unwrap().total_cost)
        );
        pass_to_resolution(&mut game);
        let stone = game.object_id(CardId(1)).unwrap();
        assert_eq!(game.zone(stone), Some(Zone::Battlefield));
        assert_eq!(game.mana(player, Color::C), Some(0.into()));

        assert!(matches!(
            answer(&mut game, Response::Cast { card: CardId(2) }),
            Request::Mana { .. }
        ));
        for source in [stone, orchard] {
            assert_eq!(game.mana_abilities(source).unwrap(), [Color::G]);
            let next = answer(&mut game, Response::ActivateMana { object: source });
            assert!(matches!(next, Request::ManaColor { colors, .. } if colors == [Color::G]));
            assert!(game.current(source).unwrap().tapped);
            assert_eq!(game.state.rules.stack.len(), 1);
            if source == stone {
                let mut departed = game.fork();
                departed.concede(PlayerId(1)).unwrap();
                assert!(matches!(request(&mut departed), Request::Mana { .. }));
                assert!(departed.current(stone).unwrap().tapped);
                assert_eq!(departed.mana(player, Color::G), Some(0.into()));
                assert!(matches!(
                    answer(&mut departed, Response::FinishMana { undo_mana: true }),
                    Request::Priority { .. }
                ));
                assert!(!departed.current(stone).unwrap().tapped);
            }
            assert!(matches!(
                answer(&mut game, Response::ManaColor { color: Color::G }),
                Request::Mana { .. }
            ));
        }
        assert_eq!(game.mana(player, Color::G), Some(2.into()));
        assert!(matches!(
            answer(&mut game, Response::FinishMana { undo_mana: true }),
            Request::SpendMana { .. }
        ));
        let spending = BTreeMap::from([(Color::G, 2.into())]);
        answer(
            &mut game,
            Response::SpendMana {
                spending: spending.clone(),
            },
        );
        assert_eq!(
            game.top().unwrap().paid.as_ref(),
            Some(&game.top().unwrap().total_cost)
        );
        assert_eq!(game.mana(player, Color::G), Some(0.into()));
        assert!(game.events().iter().any(|event| matches!(
            event,
            Event::Committed { actual: EventOutcome::ManaPaidSelected { cost, spending: actual, .. }, .. }
                if cost.generic == Quantity::from(1) && cost.colored == [Color::G] && *actual == spending
        )));
        assert!(matches!(
            pass_to_resolution(&mut game),
            Request::Priority { .. }
        ));
        assert_eq!(
            game.zone(game.object_id(CardId(2)).unwrap()),
            Some(Zone::Battlefield)
        );
        assert!(game.state.rules.stack.is_empty());
    }

    #[test]
    fn opponent_land_mana_empty_cycle_and_grounded_propagation() {
        let mut rows = [
            ("Exotic Orchard", 0, false),
            ("Exotic Orchard", 1, false),
            ("Birds of Paradise", 2, false),
            ("Forest", 3, true),
        ];
        let mut empty = Game::with_trace(prepared(&rows), TraceMode::Detailed);
        let orchard = empty.object_id(CardId(0)).unwrap();
        assert!(empty.mana_abilities(orchard).unwrap().is_empty());
        assert!(matches!(
            answer(&mut empty, Response::ActivateMana { object: orchard }),
            Request::Priority { .. }
        ));
        assert!(empty.current(orchard).unwrap().tapped);
        assert!(!empty.events().iter().any(|event| matches!(
            event,
            Event::Committed {
                actual: EventOutcome::ManaAdded { .. },
                ..
            }
        )));

        rows[3].2 = false;
        let grounded = Game::new(prepared(&rows));
        // Seat 3's own Forest can reach seat 3's answer only through a
        // dynamic land controlled by an opponent.
        assert_eq!(
            grounded.opponent_land_mana_colors(PlayerId(3)).unwrap(),
            [Color::G]
        );
        assert_eq!(grounded.mana_abilities(orchard).unwrap(), [Color::G]);
    }
}
