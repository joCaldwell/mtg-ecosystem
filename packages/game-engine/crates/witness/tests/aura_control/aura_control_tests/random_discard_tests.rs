//! Paid random discard from a private seeded pod. CR 608.2c and 701.9a–b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const CASTER: PlayerId = PlayerId(0);
const TARGET: PlayerId = PlayerId(1);
const SIGN_IN_BLOOD: &str = "c6207f6a-a624-4754-88f5-dbe700c841ff";

fn discard_spell(catalog: &Catalog) -> Definition {
    if let Some(path) = std::env::var_os("MTG_RANDOM_DISCARD_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen random-discard probe");
        let hymn = archive
            .lines()
            .find(|row| {
                sha256(row.as_bytes())
                    == "51eba19037e55f546977cc1760fd0a43e5854896f65b8fca909425945e11d2f4"
            })
            .expect("pinned complete Hymn to Tourach");
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_witness::compile(
            include_str!("../../../../../tests/fixtures/archive-manifest-current.json"),
            hymn,
        ) else {
            panic!("Hymn to Tourach must lower");
        };
        return *definition;
    }
    let mut spell = catalog
        .definition_by_oracle_id(SIGN_IN_BLOOD)
        .unwrap()
        .clone();
    spell.name = "Synthetic random discard".into();
    spell.oracle_id = "synthetic-random-discard".into();
    spell.source_labels = None;
    spell.spell.as_mut().unwrap().instructions = vec![Instruction::DiscardTargetPlayerAtRandom {
        target_slot: 0,
        count: RandomDiscardCount::Fixed { value: 2.into() },
        source_block: 0,
    }];
    spell.capabilities = spell.requirements();
    spell
}

fn seeded_pod() -> (Prepared, CardId) {
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap();
    let spell = discard_spell(&catalog);
    assert!(
        spell
            .capabilities
            .contains(&Capability::RandomPlayerDiscardV1)
    );
    seeded_pod_with_card(catalog, spell)
}

/// Keep the reviewed deck layout and real opening shuffle for private witnesses
/// of one complete replacement definition. Admission data stays unchanged.
pub(super) fn seeded_pod_with_card(catalog: Catalog, spell: Definition) -> (Prepared, CardId) {
    let mut seed = [0; 32];
    seed[0] = 3; // The real opening shuffle draws seat zero's singleton spell.
    let mut config = MatchConfig::for_profile("commander-staples-v1", seed, CASTER).unwrap();
    config.decks[0] = Decklist {
        commander: YARGLE.into(),
        cards: [(YARGLE, 1), (SWAMP, 98), (SIGN_IN_BLOOD, 1)]
            .map(|(id, count)| DeckEntry {
                oracle_id: id.into(),
                count,
            })
            .into(),
    };
    let mut pod = Prepared::admit_pod(catalog, config).unwrap();

    // Substitute one spell in the private fixture before Game creation. Keep the
    // admitted physical layout, deck counts and seeded setup; the pool stays pinned.
    let match_config = pod.match_config.as_mut().unwrap();
    let row = match_config.decks[0]
        .cards
        .iter_mut()
        .find(|row| row.oracle_id == SIGN_IN_BLOOD)
        .unwrap();
    row.oracle_id = spell.oracle_id.clone();
    let card = pod
        .config
        .cards
        .iter_mut()
        .find(|c| c.owner == CASTER && c.oracle_id == SIGN_IN_BLOOD)
        .unwrap();
    card.oracle_id = spell.oracle_id.clone();
    let source = card.id;
    let catalog = Arc::make_mut(&mut pod.catalog);
    catalog.definitions.push(spell);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&(&pod.config, &pod.match_config)).unwrap();
    (pod, source)
}

fn cast_on_second_turn(g: &mut Game, source: CardId) {
    let mut lands_played = 0;
    for _ in 0..512 {
        let r = request(g, 128);
        let response = if matches!(r, Request::Mulligan { .. }) {
            Response::Mulligan { take: false }
        } else if g.main_phase_action(CASTER) && g.turn().step == TurnStep::PrecombatMain {
            let hand = g.action_hand(CASTER);
            assert!(
                hand.iter().any(|o| o.card == source),
                "seeded spell in hand"
            );
            if lands_played == 2 {
                break;
            }
            if g.turn().lands_played == 0 {
                let land = hand
                    .iter()
                    .find(|o| g.state.rules.cards[&o.card].oracle_id == SWAMP)
                    .unwrap();
                lands_played += 1;
                Response::PlayLand { card: land.card }
            } else {
                Response::Pass
            }
        } else {
            pass(&r)
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert_eq!(lands_played, 2);
    answer(g, 128, Response::Cast { card: source });
    answer(
        g,
        128,
        Response::PlayerTargets {
            players: vec![TARGET],
        },
    );
    fund(g, 128, "Swamp", 2);
    answer(g, 128, Response::FinishMana { undo_mana: false });
    answer(
        g,
        128,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::B, 2.into())]),
        },
    );
    assert!(matches!(request(g, 128), Request::Priority { .. }));
    let entry = g.top().unwrap();
    assert_eq!(entry.source.card, source);
    assert_eq!(entry.player_targets, [TARGET]);
    assert_eq!(entry.paid.as_ref(), Some(&entry.total_cost));
    assert_eq!(entry.total_cost.colored, [Color::B, Color::B]);
}

fn word_position(g: &Game) -> RngPosition {
    g.state
        .rules
        .match_state
        .as_ref()
        .unwrap()
        .random
        .word_position
}

#[test]
fn random_discard_paid_seeded_subset_restores_between_samples() {
    let (pod, source) = seeded_pod();
    let mut game = Game::with_trace(pod.clone(), TraceMode::Off);
    cast_on_second_turn(&mut game, source);
    let mut hands: [_; 4] = std::array::from_fn(|seat| game.action_hand(PlayerId(seat as u8)));
    let hand = &hands[TARGET.0 as usize];
    assert_eq!(hand.len(), 7);
    // The four opening shuffles use 392 words. Seed 3's next two words are
    // 997694279 and 1242951482: ranks 1 of 7, then 2 of the remaining 6.
    let discarded = [hand[1], hand[3]];
    assert_eq!(word_position(&game), RngPosition(392));
    for _ in 0..4 {
        answer(&mut game, 128, Response::Pass);
    }
    for _ in 0..32 {
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        if let Frame::Resolve { frame } = &game.state.rules.frame
            && let Some(progress) = &frame.random_discard
            && !progress.selected.is_empty()
        {
            assert_eq!(progress.selected, [discarded[0]]);
            break;
        }
    }
    assert_eq!(word_position(&game), RngPosition(393));
    assert_eq!(game.action_hand(TARGET), *hand);
    let mut restored = Game::restore(pod, &game.snapshot().unwrap()).unwrap();
    assert_eq!(game.state, restored.state);
    hands[TARGET.0 as usize].retain(|object| !discarded.contains(object));
    for (g, budget) in [(&mut game, 1), (&mut restored, 128)] {
        assert!(matches!(request(g, budget), Request::Priority { .. }));
        assert!(g.state.rules.stack.is_empty());
        for (seat, expected) in hands.iter().enumerate() {
            assert_eq!(g.action_hand(PlayerId(seat as u8)), *expected);
        }
        for old in discarded {
            assert_eq!(
                g.zone(g.object_id(old.card).unwrap()),
                Some(Zone::Graveyard { owner: TARGET })
            );
        }
        assert_eq!(word_position(g), RngPosition(394));
    }
    assert_eq!(game.state, restored.state);
}
