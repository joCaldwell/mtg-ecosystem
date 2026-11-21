//! CR 118.12a, 603.3a, 605.3a and 608.2d (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::targeted_discard_tests::public_answer;
use super::*;
use std::io::{BufRead, BufReader};

const CASTER: PlayerId = PlayerId(0);
const CONTROLLER: PlayerId = PlayerId(1);
const OWNER: PlayerId = PlayerId(2);

fn scenario() -> Prepared {
    let row = if let Some(path) = std::env::var_os("MTG_CAST_PAYER_DRAW_ARCHIVE_PROBE") {
        let mut lines = BufReader::new(std::fs::File::open(path).unwrap()).lines();
        let first = lines.next().unwrap().unwrap();
        if serde_json::from_str::<serde_json::Value>(&first).unwrap()["recordType"] == "manifest" {
            lines
                .nth(12509 - 2)
                .expect("missing physical row 12509")
                .unwrap()
        } else {
            assert!(
                lines.next().is_none(),
                "expected full archive or one extracted row"
            );
            first
        }
    } else {
        include_str!(
            "../../../../../tests/fixtures/ramos/53236dd7-845a-444c-96d5-f41ed7325d8f.json"
        )
        .trim_end()
        .to_owned()
    };
    assert_eq!(
        sha256(row.as_bytes()),
        "0e5a727566c9304a0694c2dd90ab89739e8e973521d0de53a75617a43a4b2aa0"
    );
    prepared_with_sources(
        &[
            ("Rhystic Study", OWNER.0, CONTROLLER.0, false),
            ("Sol Ring", CASTER.0, CASTER.0, true),
            ("Forest", CASTER.0, CASTER.0, false),
            ("Birds of Paradise", CASTER.0, CASTER.0, false),
        ],
        &[&row],
    )
}

#[test]
fn cast_payer_draw_paid_cast_and_controller_option() {
    let mut g = Game::with_trace(scenario(), TraceMode::Detailed);
    announce(&mut g, 1, 1, vec![]);
    let [spell, trigger] = g.state.rules.stack.as_slice() else {
        panic!("one cast and one trigger")
    };
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    assert_eq!(trigger.kind.event_player().unwrap(), Some(CASTER));
    assert_eq!(trigger.controller, CONTROLLER);
    let spell = spell.clone();
    let library = g.state.rules.libraries[&CONTROLLER].len();
    for _ in 0..4 {
        answer(&mut g, 1, Response::Pass);
    }
    assert!(matches!(
        request(&mut g, 1),
        Request::OptionalEffect { player: CASTER, .. }
    ));
    assert!(g.request_header(CONTROLLER).unwrap().is_none());

    for try_payment in [false, true] {
        let mut unpaid = g.fork();
        public_answer(
            &mut unpaid,
            CASTER,
            PlayerResponse::OptionalEffect {
                perform: try_payment,
            },
        );
        if try_payment {
            // The Forest paid for the spell. Finish without activating Birds.
            public_answer(
                &mut unpaid,
                CASTER,
                PlayerResponse::FinishMana { undo_mana: false },
            );
        }
        assert!(matches!(
            request(&mut unpaid, 1),
            Request::OptionalEffect {
                player: CONTROLLER,
                ..
            }
        ));
        assert!(unpaid.request_header(CASTER).unwrap().is_none());
        public_answer(
            &mut unpaid,
            CONTROLLER,
            PlayerResponse::OptionalEffect { perform: true },
        );
        assert!(matches!(request(&mut unpaid, 1), Request::Priority { .. }));
        assert_eq!(unpaid.state.rules.libraries[&CONTROLLER].len(), library - 1);
        assert_eq!(unpaid.top().unwrap(), &spell);
    }

    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::OptionalEffect { perform: true },
    );
    let birds = g.handle_for(CASTER, object(&g, 3)).unwrap();
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::ActivateMana { object: birds },
    );
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::ManaColor { color: Color::U },
    );

    // The source owner leaves with a color response already accepted. The
    // paid mana ability and the frozen trigger continue with their own actors.
    g.concede(OWNER).unwrap();
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::FinishMana { undo_mana: false },
    );
    assert!(
        matches!(request(&mut g, 1), Request::SpendMana { player: CASTER, cost, .. }
        if cost.generic == 1.into() && cost.colored.is_empty())
    );
    assert!(g.object_id(CardId(0)).is_none());
    let mut g = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
    public_answer(
        &mut g,
        CASTER,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );

    let mut departed = g.fork();
    departed.concede(CASTER).unwrap();
    assert!(matches!(
        request(&mut departed, 1),
        Request::OptionalEffect {
            player: CONTROLLER,
            ..
        }
    ));
    public_answer(
        &mut departed,
        CONTROLLER,
        PlayerResponse::OptionalEffect { perform: false },
    );
    assert!(matches!(
        request(&mut departed, 1),
        Request::Priority { .. }
    ));
    assert_eq!(departed.state.rules.libraries[&CONTROLLER].len(), library);

    assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
    assert_eq!(g.top().unwrap(), &spell);
    assert_eq!(g.state.rules.libraries[&CONTROLLER].len(), library);
    assert!(
        g.state.rules.players[CASTER.0 as usize]
            .mana
            .values()
            .all(|n| n == &Quantity::zero())
    );
    assert!(g.current(object(&g, 3)).unwrap().tapped);
}
