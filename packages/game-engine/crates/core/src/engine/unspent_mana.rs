//! Keep unspent mana in black at the checked step/phase boundary.
//! CR 106.4, 500.5 and 614.1a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    pub(super) fn has_unspent_mana_converter(&self) -> bool {
        self.prepared.catalog.definitions.iter().any(|d| {
            d.capabilities
                .contains(&Capability::UnspentManaConversionV1)
        })
    }

    pub(super) fn unspent_mana_converters(&self, player: PlayerId) -> Vec<ObjectId> {
        if !self.has_unspent_mana_converter() {
            return vec![];
        }
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.controller == player
                    && o.present_on_battlefield()
                    && self.is_live(self.state.rules.cards[&o.id.card].owner)
                    && self.definition(o.id.card).is_ok_and(|d| {
                        d.static_abilities.iter().any(|a| {
                            a.functioning_zone == FunctioningZone::Battlefield
                                && a.effect == StaticEffect::KeepUnspentManaAsBlack
                        })
                    })
            })
            .map(|o| o.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::equipped_indestructible_tests::{answer, recover, request};
    use serde_json::{Value, json};

    fn game() -> Game {
        let history =
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
        let mut prepared = Prepared::admit(history, ScenarioConfig::land_return()).unwrap();
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let mut converter = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap()
            .clone();
        converter.oracle_id = "synthetic-pool-converter".into();
        converter.name = "Pool Converter".into();
        converter.source_labels = None;
        let mut source: Value =
            mtg_contracts::strict_read(&converter.provenance.source_json).unwrap();
        source["name"] = json!(converter.name);
        source["oracle_id"] = json!(converter.oracle_id);
        source["oracle_text"] = json!(format!(
            "If you would lose unspent mana, that mana becomes {} instead.",
            "black"
        ));
        converter.provenance.source_json = canonical(&source).unwrap();
        converter.provenance.source_sha256 = sha256(converter.provenance.source_json.as_bytes());
        converter.provenance.abilities_json = canonical(&json!([{
            "kind":"static","effect":{"static":"replacement","replacement":{
                "event":{"event":"lose-unspent-mana","who":{"player":"you"}},
                "occurrence":"each","replace":{"replacement":"keep-mana","as":{"symbol":"B"}}
            }}
        }]))
        .unwrap();
        converter.provenance.ast_sha256 =
            sha256(format!("[{}]", converter.provenance.abilities_json).as_bytes());
        converter.static_abilities = vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::KeepUnspentManaAsBlack,
        }];
        converter.capabilities = converter.requirements();
        converter.validate().unwrap();
        let id = converter.oracle_id.clone();
        catalog.definitions.push(converter);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        let p0 = PlayerId(0);
        let land = |id, name| CardSetup {
            id: CardId(id),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: p0,
            controller: p0,
            zone: Zone::Battlefield,
        };
        prepared.config.cards = vec![
            land(0, "Island"),
            land(1, "Forest"),
            CardSetup {
                id: CardId(2),
                oracle_id: id,
                owner: PlayerId(1),
                controller: p0,
                zone: Zone::Battlefield,
            },
        ];
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared.profile_digest = digest(&prepared.config).unwrap();
        Game::with_trace(prepared, TraceMode::Detailed)
    }

    fn next_step(g: &mut Game) {
        let old = g.state.rules.turn.step;
        for _ in 0..8 {
            let r = request(g, 1);
            if g.state.rules.turn.step != old {
                return;
            }
            assert!(matches!(r, Request::Priority { .. }), "{r:?}");
            g.respond(r.player(), r.decision_id(), Response::Pass)
                .unwrap();
        }
        panic!("expected ordinary next step");
    }

    #[test]
    fn unspent_mana_converts_at_real_boundary_and_ends_with_grant() {
        let mut g = game();
        for card in [CardId(0), CardId(1)] {
            let object = g.object_id(card).unwrap();
            answer(&mut g, 1, Response::ActivateMana { object });
        }
        request(&mut g, 1);
        let p0 = PlayerId(0);
        assert_eq!(g.mana(p0, Color::U), Some(1.into()));
        assert_eq!(g.mana(p0, Color::G), Some(1.into()));
        let mut departed = g.fork();
        departed.concede(PlayerId(1)).unwrap();
        next_step(&mut departed);
        assert!(departed.state.rules.players[0].mana.is_empty());
        recover(&departed);

        next_step(&mut g);
        assert_eq!(
            g.state.rules.players[0].mana,
            BTreeMap::from([(Color::B, 2.into())])
        );
        assert!(g.trace().records.iter().any(|r| matches!(&r.record,
        EvidenceRecord::Batch { batch } if batch.occurrences.iter().any(|o|
            matches!(o.actual, EventOutcome::ManaCleared {
                player: PlayerId(0), retained_as: Some(Color::B), ..
            })))));
        recover(&g);
    }
}
