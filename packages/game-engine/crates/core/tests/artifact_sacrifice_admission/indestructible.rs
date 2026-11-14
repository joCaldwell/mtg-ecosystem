//! Accepted-input paid pod; synthetic policy observations live in sim tests.
use super::*;
pub(super) const RELIC: &str = "2bda1fb1-528c-4e66-89f1-bb1a85e9181e";
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let mut m = super::config(c, seed);
    m.decks[0]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == oracle(c, "Sol Ring"))
        .unwrap()
        .oracle_id = RELIC.into();
    for deck in &mut m.decks[1..] {
        let cmd = oracle(c, "Isamaru, Hound of Konda");
        *deck = Decklist {
            commander: cmd.clone(),
            cards: vec![
                DeckEntry {
                    oracle_id: cmd,
                    count: 1,
                },
                DeckEntry {
                    oracle_id: oracle(c, "Plains"),
                    count: 99,
                },
            ],
        };
    }
    validate_pod(c, &m).unwrap();
    m
}
fn seed(index: u32) -> [u8; 32] {
    let hash = sha256(
        format!("commander-sacrifice-v1/opening-indestructible-shrapnel/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).unwrap())
}
pub(super) fn opening(g: &Game, c: &Catalog) -> bool {
    let v = observation(g, PlayerId(0)).view;
    [RELIC, SHRAPNEL].iter().all(|id| {
        v.objects
            .iter()
            .any(|o| o.oracle_id == *id && o.zone == (Zone::Hand { owner: PlayerId(0) }))
    }) && c
        .definition_by_oracle_id(RELIC)
        .unwrap()
        .characteristics
        .mana_cost
        == ManaCost {
            generic: 0.into(),
            colored: vec![],
        }
}
#[test]
#[ignore = "bounded setup-only scan; keeps four hands and stops at Upkeep"]
fn indestructible_sacrifice_setup_only_seed_probe() {
    let c = catalog();
    for index in 0..2048 {
        let (_, g) = setup_config(&c, config(&c, seed(index)), 4096);
        if opening(&g, &c) {
            println!(
                "first opening-only success index={index} seed={:?}",
                seed(index)
            );
            return;
        }
    }
    panic!("no opening in setup-only 0..2048");
}
#[test]
fn indestructible_sacrifice_paid_pod_replays_across_quanta() {
    let c = catalog();
    // First opening in the retained bounded 0..2048 setup-only scan.
    let seed = seed(460);
    assert_eq!(
        seed,
        [
            227, 77, 160, 145, 181, 149, 127, 161, 222, 122, 172, 104, 6, 69, 255, 170, 232, 234,
            190, 187, 4, 196, 171, 45, 60, 234, 112, 33, 219, 161, 75, 146
        ]
    );
    let a = run_payment(&c, config(&c, seed), "Darksteel Relic", 1);
    let b = run_payment(&c, config(&c, seed), "Darksteel Relic", 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
