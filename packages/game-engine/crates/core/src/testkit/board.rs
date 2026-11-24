//! Immutable frozen catalogs and fresh private boards for white-box witnesses.
use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum FixtureCatalog {
    Base,
    Action,
    Match,
    Witness,
}

impl FixtureCatalog {
    pub fn admit(self, config: ScenarioConfig) -> Prepared {
        Prepared::admit(self.get().clone(), config).unwrap()
    }

    pub fn get(self) -> &'static Catalog {
        macro_rules! catalog {
            ($file:literal) => {{
                static CATALOG: OnceLock<Catalog> = OnceLock::new();
                CATALOG.get_or_init(|| {
                    Catalog::read(include_str!(concat!("../../../../tests/fixtures/", $file)))
                        .unwrap()
                })
            }};
        }
        match self {
            Self::Base => catalog!("expected-ir.json"),
            Self::Action => catalog!("expected-action-ir.json"),
            Self::Match => catalog!("expected-match-ir.json"),
            Self::Witness => catalog!("expected-witness-r4-ir.json"),
        }
    }
}

pub fn oracle_id<'a>(catalog: &'a Catalog, name: &str) -> &'a str {
    &catalog
        .definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("missing fixture card {name:?}"))
        .oracle_id
}

pub struct Board {
    catalog: Catalog,
    config: ScenarioConfig,
}

impl Board {
    pub fn new(catalog: Catalog, profile: &str) -> Self {
        Self {
            catalog,
            config: ScenarioConfig {
                initial: ScenarioInitial::default(),
                mode: Mode::Scenario,
                players: 4,
                profile: profile.into(),
                cards: Vec::new(),
                commanders: Vec::new(),
            },
        }
    }

    pub fn seats(mut self, players: u8) -> Self {
        self.config.players = players;
        self
    }

    pub fn life(mut self, seat: PlayerId, life: Quantity) -> Self {
        self.config.initial.life[usize::from(seat.0)] = life;
        self
    }

    pub fn step(mut self, step: TurnStep) -> Self {
        self.config.initial.turn_step = step;
        self
    }

    pub fn card(mut self, name: &str, owner: u8, controller: u8, zone: Zone) -> Self {
        self.config.cards.push(CardSetup {
            id: CardId(u32::try_from(self.config.cards.len()).unwrap()),
            oracle_id: oracle_id(&self.catalog, name).into(),
            owner: PlayerId(owner),
            controller: PlayerId(controller),
            zone,
        });
        self
    }

    pub fn permanent_or_hand(self, name: &str, owner: u8, controller: u8, hand: bool) -> Self {
        self.card(
            name,
            owner,
            controller,
            if hand {
                Zone::Hand {
                    owner: PlayerId(owner),
                }
            } else {
                Zone::Battlefield
            },
        )
    }

    pub fn libraries(mut self, name: &str, count: usize) -> Self {
        for seat in 0..self.config.players {
            for _ in 0..count {
                self = self.card(
                    name,
                    seat,
                    seat,
                    Zone::Library {
                        owner: PlayerId(seat),
                    },
                );
            }
        }
        self
    }

    pub fn commander(mut self, id: CardId) -> Self {
        self.config.commanders.push(id);
        self
    }

    pub fn prepare(self) -> Prepared {
        witness_scenario(self.catalog, self.config)
    }
}
