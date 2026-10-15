//! Exact reviewed configurations, not arbitrary setup or deck admission.
//! Setup facts are bound by the profile identity and complete config equality.
//! See docs/game-engine-combat-games.md for the frozen sources and policies.
use super::*;

impl ScenarioConfig {
    pub fn command_casting() -> Self {
        let cards = (0..14)
            .map(|id| {
                let owner = PlayerId(match id {
                    0..=5 | 13 => 0,
                    6 | 7 | 10 => 1,
                    8 | 11 => 2,
                    _ => 3,
                });
                CardSetup {
                    id: CardId(id),
                    owner,
                    controller: owner,
                    oracle_id: match id {
                        0 => ISAMARU,
                        1..=5 | 13 => PLAINS,
                        7 => SPELL,
                        _ => ISLAND,
                    }
                    .into(),
                    zone: match id {
                        0 => Zone::Command { owner },
                        7 => Zone::Hand { owner },
                        10..=13 => Zone::Library { owner },
                        _ => Zone::Battlefield,
                    },
                }
            })
            .collect();
        Self {
            mode: Mode::Scenario,
            players: 4,
            profile: "four-seat-commander-casting-v1".into(),
            commanders: vec![CardId(0)],
            cards,
        }
    }
    pub fn commander_lifecycle() -> Self {
        let mut config = Self::command_casting();
        config.profile = "four-seat-commander-lifecycle-v1".into();
        config.cards[0].zone = Zone::Battlefield;
        config.cards.push(CardSetup {
            id: CardId(14),
            oracle_id: BEARS.into(),
            owner: PlayerId(1),
            controller: PlayerId(1),
            zone: Zone::Battlefield,
        });
        config
    }
    pub fn commander_exile() -> Self {
        let mut config = Self::commander_lifecycle();
        config.profile = "four-seat-commander-exile-v1".into();
        config.cards[0].controller = PlayerId(1);
        config
    }
    pub(super) fn commander_profile(&self) -> bool {
        matches!(
            self.profile.as_str(),
            "four-seat-commander-casting-v1"
                | "four-seat-commander-lifecycle-v1"
                | "four-seat-commander-exile-v1"
        )
    }

    pub fn combat_game() -> Self {
        let cards = (0..27)
            .map(|id| {
                let owner = PlayerId(match id {
                    0..=6 => 0,
                    7..=10 => 1,
                    11..=16 => 2,
                    _ => 3,
                });
                let oracle_id = match id {
                    1 | 3 | 8 | 12 | 14 => BEARS,
                    9 | 18 => SPELL,
                    7 | 10 | 17 | 19..=26 => ISLAND,
                    _ => FOREST,
                };
                let zone = match id {
                    0 | 1 | 7 | 8 | 11 | 12 | 17 => Zone::Battlefield,
                    4..=6 | 10 | 15 | 16 | 26 => Zone::Library { owner },
                    _ => Zone::Hand { owner },
                };
                CardSetup {
                    id: CardId(id),
                    oracle_id: oracle_id.into(),
                    owner,
                    controller: owner,
                    zone,
                }
            })
            .collect();
        Self {
            mode: Mode::Scenario,
            players: 4,
            profile: "four-seat-combat-game-v1".into(),
            commanders: vec![],
            cards,
        }
    }

    pub fn combat_interactions() -> Self {
        let cards = (0..8)
            .map(|id| {
                let owner = PlayerId(match id {
                    0..=2 => 0,
                    3 | 4 => 1,
                    5 => 2,
                    _ => 3,
                });
                CardSetup {
                    id: CardId(id),
                    oracle_id: match id {
                        6 => ISLAND,
                        7 => SPELL,
                        _ => BEARS,
                    }
                    .into(),
                    owner,
                    controller: owner,
                    zone: if id == 7 {
                        Zone::Hand { owner }
                    } else {
                        Zone::Battlefield
                    },
                }
            })
            .collect();
        Self {
            mode: Mode::Scenario,
            players: 4,
            profile: "four-seat-combat-interactions-v1".into(),
            commanders: vec![],
            cards,
        }
    }

    pub(super) fn combat_profile(&self) -> bool {
        matches!(
            self.profile.as_str(),
            "four-seat-combat-game-v1"
                | "four-seat-combat-interactions-v1"
                | "four-seat-commander-casting-v1"
                | "four-seat-commander-lifecycle-v1"
                | "four-seat-commander-exile-v1"
        )
    }

    pub(super) fn initial_life(&self, player: PlayerId) -> Quantity {
        Quantity::from(if self.combat_profile() && !self.commander_profile() {
            if player == PlayerId(0) { 6 } else { 2 }
        } else {
            40
        })
    }

    pub(super) fn initial_turn_step(&self) -> TurnStep {
        if self.combat_profile() {
            TurnStep::PrecombatMain
        } else {
            TurnStep::PostcombatMain
        }
    }

    pub(super) fn initial_libraries(&self) -> BTreeMap<PlayerId, Vec<CardId>> {
        if self.profile == "four-seat-combat-game-v1" {
            return BTreeMap::from([
                (PlayerId(0), vec![CardId(6), CardId(4), CardId(5)]),
                (PlayerId(1), vec![CardId(10)]),
                (PlayerId(2), vec![CardId(16), CardId(15)]),
                (PlayerId(3), vec![CardId(26)]),
            ]);
        }
        // Older exact configurations have at most one card in each library;
        // iteration order therefore cannot choose their next draw. The new
        // interactions configuration deliberately has four empty libraries.
        (0..self.players)
            .map(|seat| {
                let owner = PlayerId(seat);
                let cards = self
                    .cards
                    .iter()
                    .filter(|card| card.zone == (Zone::Library { owner }))
                    .map(|card| card.id)
                    .collect::<Vec<_>>();

                (owner, cards)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap()
    }

    #[test]
    fn frozen_combat_setup_has_explicit_order_and_monochromatic_mana() {
        let catalog = catalog();
        for config in [
            ScenarioConfig::combat_game(),
            ScenarioConfig::combat_interactions(),
        ] {
            assert_eq!(config.initial_turn_step(), TurnStep::PrecombatMain);
            assert!(config.commanders.is_empty());
            let libraries = config.initial_libraries();
            for seat in 0..4 {
                let player = PlayerId(seat);
                assert_eq!(
                    config.initial_life(player),
                    Quantity::from(if seat == 0 { 6 } else { 2 })
                );
                let owned = config
                    .cards
                    .iter()
                    .filter(|card| card.owner == player)
                    .collect::<Vec<_>>();
                assert!(owned.len() <= 16);
                assert!(owned.iter().all(|card| card.controller == player));
                let actual = owned
                    .iter()
                    .filter(|card| card.zone == (Zone::Library { owner: player }))
                    .map(|card| card.id)
                    .collect::<BTreeSet<_>>();
                let explicit = libraries[&player].iter().copied().collect::<BTreeSet<_>>();
                assert_eq!(actual, explicit);
                assert_eq!(explicit.len(), libraries[&player].len());
                let colors = owned
                    .iter()
                    .filter_map(|card| {
                        let definition = catalog
                            .definitions
                            .iter()
                            .find(|d| d.oracle_id == card.oracle_id)
                            .unwrap();
                        (definition.characteristics.kind == CardKind::Land).then(|| {
                            basic_land_color(&definition.characteristics.subtypes[0]).unwrap()
                        })
                    })
                    .collect::<BTreeSet<_>>();
                assert!(
                    colors.len() <= 1,
                    "generic payment must have no color choice"
                );
                assert!(
                    colors
                        .iter()
                        .all(|color| *color == if seat % 2 == 0 { Color::G } else { Color::U })
                );
            }
        }
        let game = ScenarioConfig::combat_game();
        assert_eq!(game.cards.len(), 27);
        assert_eq!(
            game.initial_libraries()[&PlayerId(0)],
            vec![CardId(6), CardId(4), CardId(5)]
        );
        assert_eq!(
            game.initial_libraries()[&PlayerId(2)],
            vec![CardId(16), CardId(15)]
        );
        assert_eq!(ScenarioConfig::combat_interactions().cards.len(), 8);
    }

    #[test]
    fn combat_admission_rejects_every_unreviewed_setup_mutation() {
        for config in [
            ScenarioConfig::combat_game(),
            ScenarioConfig::combat_interactions(),
        ] {
            assert!(Prepared::admit(catalog(), config.clone()).is_ok());
            type Mutation = fn(&mut ScenarioConfig);
            let mutations: Vec<Mutation> = vec![
                |c| c.mode = Mode::CommanderMatch,
                |c| c.players = 3,
                |c| c.profile.push_str("-unreviewed"),
                |c| c.commanders.push(CardId(1)),
                |c| c.cards[0].controller = PlayerId(1),
                |c| c.cards[0].owner = PlayerId(1),
                |c| c.cards[0].zone = Zone::Hand { owner: PlayerId(0) },
                |c| c.cards[0].oracle_id = ISLAND.into(),
                |c| c.cards[0].id = CardId(99),
                |c| {
                    c.cards.pop();
                },
                |c| c.cards.push(c.cards[0].clone()),
                |c| c.cards.reverse(),
            ];
            for mutation in mutations {
                let mut changed = config.clone();
                mutation(&mut changed);
                assert!(Prepared::admit(catalog(), changed).is_err());
            }
            let mut changed_catalog = catalog();
            let bears = changed_catalog
                .definitions
                .iter_mut()
                .find(|d| d.oracle_id == BEARS)
                .unwrap();
            bears.characteristics.power = Some(Quantity::from(3));
            // Structurally valid lowering is not exact-definition admission.
            changed_catalog.validate().unwrap();
            assert!(Prepared::admit(changed_catalog, config).is_err());
        }
    }
}
