//! CR 121.2c, 704.5b, 800.4 and 903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::targeted_discard_tests::public_answer;
use super::*;

const ACTIVE: PlayerId = PlayerId(1);
const CASTER: PlayerId = PlayerId(2);

fn scenario() -> Prepared {
    let mut rows = [
        ("Aetherize", 2, 2, true),
        ("Island", 2, 2, false),
        ("Island", 2, 2, false),
        ("Caelorna, Coral Tyrant", 2, 2, true),
    ];
    let mut pod = if let Some(path) = std::env::var_os("MTG_EACH_PLAYER_DRAW_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen each-player draw probe");
        let row = archive
            .lines()
            .find(|line| {
                sha256(line.as_bytes())
                    == "6396d0c1b0870998b12b5e755702eae14f6ca027021b51c4ff8d47252cf06ee7"
            })
            .expect("pinned Vision Skeins");
        rows[0].0 = "Vision Skeins";
        prepared_with_sources(&rows, &[row])
    } else {
        let mut pod = prepared(&rows);
        let catalog = Arc::make_mut(&mut pod.catalog);
        let spell = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Aetherize")
            .unwrap();
        spell.name = "Synthetic each-player draw".into();
        spell.oracle_id = "synthetic-each-player-draw".into();
        spell.source_labels = None;
        spell.characteristics.mana_cost.generic = 1.into();
        spell.spell.as_mut().unwrap().instructions = vec![Instruction::DrawEachPlayer {
            count: DrawEachCount::Fixed { value: 2.into() },
            source_block: 0,
        }];
        spell.capabilities = spell.requirements();
        pod.config.cards[0].oracle_id = spell.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        pod.catalog_digest = digest(catalog).unwrap();
        pod
    };
    // Seat one's turn-based draw leaves one card for the spell. Seat two's
    // first attempt will pause on its tucked commander.
    pod.config.initial.turn_step = TurnStep::End;
    let keep = pod.config.initial_libraries()[&ACTIVE][..2].to_vec();
    pod.config
        .cards
        .retain(|c| c.zone != (Zone::Library { owner: ACTIVE }) || keep.contains(&c.id));
    pod.config.cards[3].zone = Zone::Library { owner: CASTER };
    pod.config.commanders = vec![CardId(3)];
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

#[test]
fn each_player_draw_paid_nonactive_apnap_failure_and_commander_restore() {
    let pod = scenario();
    let mut g = Game::with_trace(pod.clone(), TraceMode::Detailed);
    own_main(&mut g, 128, ACTIVE.0, 1);
    assert_eq!(g.state.rules.libraries[&ACTIVE].len(), 1);
    public_answer(&mut g, ACTIVE, PlayerResponse::Pass);
    let card = g.handle_for(CASTER, object(&g, 0)).unwrap();
    public_answer(&mut g, CASTER, PlayerResponse::Cast { card });
    for id in [1, 2] {
        let object = g.handle_for(CASTER, object(&g, id)).unwrap();
        public_answer(&mut g, CASTER, PlayerResponse::ActivateMana { object });
    }
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::FinishMana { undo_mana: false },
    );
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, 2.into())]),
        },
    );
    request(&mut g, 1);
    let spell = g.top().unwrap();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    let stack_id = spell.id;
    for seat in [2, 3, 0, 1] {
        public_answer(&mut g, PlayerId(seat), PlayerResponse::Pass);
    }
    assert!(
        matches!(request(&mut g, 1), Request::CommanderReturn { player, object, .. } if player == CASTER && object.card == CardId(3))
    );
    assert!(g.is_live(ACTIVE) && g.state.rules.players[ACTIVE.0 as usize].failed_draw);
    let mut g = Game::restore(pod, &g.snapshot().unwrap()).unwrap();
    g.concede(PlayerId(3)).unwrap();
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    let mut rows = None;
    resolve(&mut g, 1, |g| {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && let [InstructionOutcome::EachPlayerDrawAttempts { players }] =
                frame.outcomes.as_slice()
        {
            assert!(g.is_live(ACTIVE));
            rows = Some(players.clone());
        }
    });
    let expected = [(1, 2, 1), (2, 2, 0), (3, 0, 0), (0, 2, 0)].map(|(seat, attempted, failed)| {
        PlayerDrawAttempts {
            player: PlayerId(seat),
            attempted: attempted.into(),
            failed: failed.into(),
        }
    });
    assert_eq!(rows.as_deref(), Some(expected.as_slice()));
    let actors: Vec<_> = batches(&g)
        .into_iter()
        .filter_map(|batch| match batch.cause {
            BatchCause::Instruction {
                stack_id: id,
                player,
                ..
            } if id == stack_id => Some(player.0),
            _ => None,
        })
        .collect();
    assert_eq!(actors, [1, 1, 2, 2, 0, 0]);
    assert!(!g.is_live(ACTIVE));
    assert_eq!(g.action_hand(CASTER).len(), 1);
    assert_eq!(g.action_hand(PlayerId(0)).len(), 2);
    assert_eq!(g.zone(object(&g, 3)), Some(Zone::Command { owner: CASTER }));
}
