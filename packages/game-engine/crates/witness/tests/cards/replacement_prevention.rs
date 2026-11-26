//! Frozen R5 witnesses for damage prevention and damage replacement:
//! static and resolved prevention, CR 615.7 shields, CR 615.12
//! unpreventable damage and the CR 616.1 affected-recipient order.
//! CR 120.4b, 120.8, 609.7a, 614.1a, 615.1, 615.7, 615.10, 615.12, 616.1e-f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Tiers, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const FOG_BANK: &str = record!("3a2bb47d-228d-499e-a34b-9e10d99e9b2c");
const FOG: &str = record!("27e9db49-7af7-4bef-ad4c-bf5dfb92030d");
const FRENZIED_BALOTH: &str = record!("6a4ef075-9254-4d36-9572-622833fae54e");
const DICTATE: &str = record!("3d960d33-623a-4415-ae00-f8cffbc15f5a");
const URZAS_ARMOR: &str = record!("e5fa232a-c8f2-4532-88bf-60d1223181b0");
const SHIELDMATES_BLESSING: &str = record!("4ed55e9c-84ea-42a3-a32f-469da56b3eb5");
const GUARD_GOMAZOA: &str = record!("7c565975-aebc-4599-ac32-5594c718e2cb");
const GALADHRIM_AMBUSH: &str = record!("3b22d21f-e19a-40df-840b-a2b7f11f79c2");
const VODALIAN_ZOMBIE: &str = record!("58e4f299-bb32-4727-8498-a364166adc38");
const KING_TCHALLA: &str = record!("fe00278d-4385-4e3f-919c-12a794fd86f5");

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen prevention witness failed: {result:?}");
    };
    definition.validate().unwrap();
    assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
    *definition
}

/// Two seats at seat 0's first main phase; `cards` are (name, seat, zone)
/// in board-id order.
fn game(cards: &[(&str, u8, Zone)]) -> Game {
    seated(2, cards)
}

fn seated(seats: u8, cards: &[(&str, u8, Zone)]) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        FOG_BANK,
        FOG,
        FRENZIED_BALOTH,
        DICTATE,
        URZAS_ARMOR,
        SHIELDMATES_BLESSING,
        GUARD_GOMAZOA,
        GALADHRIM_AMBUSH,
        VODALIAN_ZOMBIE,
        KING_TCHALLA,
    ] {
        catalog.definitions.push(lowered(raw));
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board = Board::new(catalog, "replacement-prevention")
        .step(TurnStep::PrecombatMain)
        .seats(seats);
    for &(name, seat, zone) in cards {
        board = board.card(name, seat, seat, zone);
    }
    Game::with_trace(board.libraries("Forest", 6).prepare(), TraceMode::Detailed)
}

fn bf() -> Zone {
    Zone::Battlefield
}

fn hand(seat: PlayerId) -> Zone {
    Zone::Hand { owner: seat }
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    assert_recoverable(g.into());
}

fn object(g: &Game, id: u32) -> ObjectId {
    Driver::object(g, id)
}

fn life(g: &Game, seat: PlayerId) -> Quantity {
    g.state.rules.players[seat.0 as usize].life.clone()
}

fn damage(g: &Game, id: u32) -> Quantity {
    g.current(object(g, id)).unwrap().marked_damage.clone()
}

fn zone(g: &Game, id: u32) -> Zone {
    g.state.rules.objects[&CardId(id)].zone
}

/// Cast board card `id` with `targets`, pay and resolve it.
fn cast(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    Driver::cast(g, CardId(id));
    mtg_witness::equipped::pay_to_priority(g, u64::MAX, targets);
    for _ in 0..8 {
        if g.state.rules.stack.is_empty() {
            return checked(g);
        }
        let r = Driver::request(g, u64::MAX);
        assert_eq!(r.reason, Reason::Priority);
        g.respond(r.player, r.id, mtg_witness::temporary::pass(&r))
            .unwrap();
    }
    panic!("spell did not resolve");
}

/// Seat 0 attacks seat 1 with `attackers`; seat 1 declares `blocks`
/// (blocker, attacker). Damage-effect choices go to `choose`, which picks
/// a candidate index; the walk stops at seat 0's postcombat main phase.
fn combat(
    g: &mut Game,
    attackers: &[u32],
    blocks: &[(u32, u32)],
    mut choose: impl FnMut(&Game, &Decision<Rules>) -> usize,
) -> usize {
    let mut choices = 0;
    for _ in 0..80 {
        let r = Driver::request(g, u64::MAX);
        let answer = match &r.reason {
            Reason::Attackers => r.attack(attackers.iter().map(|a| (object(g, *a), P1)).collect()),
            Reason::Blockers => r.block(
                blocks
                    .iter()
                    .map(|(b, a)| (object(g, *b), object(g, *a)))
                    .collect(),
            ),
            Reason::DamageEffect { .. } => {
                // The pending choice and its applied prefix recover.
                checked(g);
                choices += 1;
                let pick = choose(g, &r);
                r.select(r.candidates[pick].clone())
            }
            Reason::Priority if g.state.rules.turn.step == TurnStep::PostcombatMain => {
                checked(g);
                return choices;
            }
            Reason::Priority => mtg_witness::temporary::pass(&r),
            other => panic!("combat {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("combat did not finish");
}

fn no_choice(_: &Game, d: &Decision<Rules>) -> usize {
    panic!("unexpected damage-effect choice {:?}", d.candidates)
}

/// CR 615.1: "Prevent all combat damage that would be dealt to and dealt
/// by this creature" applies to both blocks; an unblocked attacker still
/// deals its damage.
#[test]
fn replacement_prevention_fog_bank_prevents_damage_to_and_by_itself() {
    let mut g = game(&[
        ("Grizzly Bears", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Fog Bank", 1, bf()),
        ("Guard Gomazoa", 1, bf()),
    ]);
    // Fog Bank (0/2) blocks Bears 0; Guard Gomazoa (1/3) blocks Bears 1:
    // only damage to Gomazoa is prevented, Gomazoa's own damage is dealt.
    combat(&mut g, &[0, 1], &[(2, 0), (3, 1)], no_choice);
    assert_eq!(damage(&g, 0), Quantity::zero());
    assert_eq!(damage(&g, 2), Quantity::zero());
    assert_eq!(damage(&g, 3), Quantity::zero());
    assert_eq!(damage(&g, 1), 1.into());
    assert_eq!(life(&g, P1), 40.into());
}

/// CR 615.12: with Frenzied Baloth, combat damage can't be prevented, so
/// Fog Bank's shield doesn't apply to the Bears' damage and Fog Bank is
/// destroyed (CR 704.5g).
#[test]
fn replacement_prevention_unpreventable_damage_bypasses_static_prevention() {
    let mut g = game(&[
        ("Frenzied Baloth", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Fog Bank", 1, bf()),
    ]);
    combat(&mut g, &[0, 1], &[(2, 1)], no_choice);
    assert_eq!(zone(&g, 2), Zone::Graveyard { owner: P1 });
    assert_eq!(life(&g, P1), 37.into());
}

/// CR 615.1, 514.2: Fog prevents every combat damage event this turn, the
/// caster's included, then expires in cleanup.
#[test]
fn replacement_prevention_fog_prevents_combat_damage_this_turn() {
    let mut g = game(&[
        ("Fog", 0, hand(P0)),
        ("Forest", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Grizzly Bears", 0, bf()),
    ]);
    cast(&mut g, 0, vec![]);
    assert_eq!(g.state.rules.effects.len(), 1);
    combat(&mut g, &[2, 3], &[], no_choice);
    assert_eq!(life(&g, P1), 40.into());
    // The effect survives combat and is removed in this turn's cleanup.
    assert_eq!(g.state.rules.effects.len(), 1);
    for _ in 0..40 {
        if g.state.rules.turn_owner == P1 {
            break;
        }
        let r = Driver::request(&mut g, u64::MAX);
        g.respond(r.player, r.id, mtg_witness::temporary::pass(&r))
            .unwrap();
    }
    assert!(g.state.rules.effects.is_empty());
    checked(&g);
}

/// CR 615.12: Fog applies to no combat damage while Frenzied Baloth says
/// it can't be prevented.
#[test]
fn replacement_prevention_unpreventable_damage_bypasses_resolved_prevention() {
    let mut g = game(&[
        ("Fog", 0, hand(P0)),
        ("Forest", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Frenzied Baloth", 0, bf()),
    ]);
    cast(&mut g, 0, vec![]);
    combat(&mut g, &[2, 3], &[], no_choice);
    assert_eq!(life(&g, P1), 35.into());
}

/// CR 615.7: "the next 3 damage" is one shield across simultaneous events.
/// With two applicable damage events the shielded player chooses which
/// one it prevents first; the shield then prevents the remainder of 3 from
/// the other and is used up.
#[test]
fn replacement_prevention_shield_depletes_across_simultaneous_events() {
    for first in [0u32, 1] {
        let mut g = game(&[
            ("Shieldmate's Blessing", 0, hand(P0)),
            ("Plains", 0, bf()),
            ("Grizzly Bears", 0, bf()),
            ("Grizzly Bears", 0, bf()),
        ]);
        cast(&mut g, 0, vec![TargetEntity::Player { player: P1 }]);
        let choices = combat(&mut g, &[2, 3], &[], |g, d| {
            assert_eq!(d.player, P1);
            assert_eq!(d.candidates.len(), 2);
            let tx = g.damage_transaction().unwrap();
            d.candidates
                .iter()
                .position(|c| {
                    matches!(c, Candidate::DamageEffect { damage, .. }
                        if tx.entries[damage.0 as usize].source.object.card == CardId(2 + first))
                })
                .unwrap()
        });
        assert_eq!(choices, 1);
        assert_eq!(life(&g, P1), 39.into());
        assert!(
            g.state.rules.effects.is_empty(),
            "the used-up shield is gone"
        );
    }
}

/// CR 615.7: a partly used shield keeps its remainder for later damage
/// this turn; a forged larger remainder is rejected on restore.
#[test]
fn replacement_prevention_shield_keeps_remainder_and_rejects_forgery() {
    let mut g = game(&[
        ("Shieldmate's Blessing", 0, hand(P0)),
        ("Plains", 0, bf()),
        ("Grizzly Bears", 0, bf()),
    ]);
    cast(&mut g, 0, vec![TargetEntity::Player { player: P1 }]);
    combat(&mut g, &[2], &[], no_choice);
    assert_eq!(life(&g, P1), 40.into());
    let [effect] = g.state.rules.effects.as_slice() else {
        panic!("shield remains")
    };
    assert!(matches!(&effect.kind,
        ResolvedEffectKind::PreventDamage { remaining: Some(r), .. } if *r == 1.into()));
    let mut forged: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    forged["state"]["effects"][0]["kind"]["remaining"] = "4".into();
    forged["state_sha256"] = mtg_contracts::digest(&forged["state"]).unwrap().into();
    let error = Game::restore(g.prepared.as_ref().clone(), &forged.to_string())
        .err()
        .expect("forged shield restored");
    assert!(
        error.contains("effect definition/provenance invariant"),
        "{error}"
    );
}

/// CR 616.1e-f: Dictate of the Twin Gods (double) and Urza's Armor
/// (prevent 1) both apply to the Bears' 2 damage to Urza's Armor's
/// controller, who chooses their order: doubling first loses 3, prevention
/// first loses 2. Each applies once (CR 614.5).
#[test]
fn replacement_prevention_affected_player_orders_replacement_and_prevention() {
    for (double_first, lost) in [(true, 3), (false, 2)] {
        let mut g = game(&[
            ("Grizzly Bears", 0, bf()),
            ("Dictate of the Twin Gods", 0, bf()),
            ("Urza's Armor", 1, bf()),
        ]);
        let choices = combat(&mut g, &[0], &[], |g, d| {
            assert_eq!(d.player, P1);
            assert_eq!(d.candidates.len(), 2);
            let dictate = object(g, 1);
            d.candidates
                .iter()
                .position(|c| {
                    matches!(c, Candidate::DamageEffect {
                        effect: EffectKey::StaticAbility { source, .. }, ..
                    } if (*source == dictate) == double_first)
                })
                .unwrap()
        });
        assert_eq!(choices, 1);
        assert_eq!(life(&g, P1), (40 - lost).into());
    }
}

/// CR 614.1a, 120.8: a doubled first damage event; Fog Bank's prevention
/// removes the event, so the doubling has nothing left to modify.
#[test]
fn replacement_prevention_doubling_has_no_event_after_full_prevention() {
    let mut g = game(&[
        ("Grizzly Bears", 0, bf()),
        ("Dictate of the Twin Gods", 0, bf()),
        ("Fog Bank", 1, bf()),
    ]);
    let choices = combat(&mut g, &[0], &[(2, 0)], |g, d| {
        // Prevent first: doubling then has no event (CR 120.8).
        let bank = object(g, 2);
        d.candidates
            .iter()
            .position(|c| {
                matches!(c, Candidate::DamageEffect {
                    effect: EffectKey::StaticAbility { source, .. }, ..
                } if *source == bank)
            })
            .unwrap()
    });
    assert_eq!(choices, 1);
    assert_eq!(damage(&g, 2), Quantity::zero());
    assert_eq!(zone(&g, 2), Zone::Battlefield);
}

/// CR 609.7a: "by non-Elf creatures" reads each source when the damage
/// would be dealt, so the attacking Bears cast before it resolved are
/// covered.
#[test]
fn replacement_prevention_resolved_source_filter_reads_damage_time_facts() {
    let mut g = game(&[
        ("Galadhrim Ambush", 1, hand(P1)),
        ("Forest", 1, bf()),
        ("Forest", 1, bf()),
        ("Forest", 1, bf()),
        ("Forest", 1, bf()),
        ("Grizzly Bears", 0, bf()),
    ]);
    let definition = g
        .prepared
        .catalog
        .definition_by_oracle_id(&lowered(GALADHRIM_AMBUSH).oracle_id)
        .unwrap()
        .clone();
    assert!(
        definition
            .capabilities
            .contains(&Capability::DamagePreventionV1)
    );
    // Seat 1 casts the Ambush in seat 0's declare-blockers step.
    for _ in 0..60 {
        let r = Driver::request(&mut g, u64::MAX);
        let answer = match &r.reason {
            Reason::Attackers => r.attack(vec![(object(&g, 5), P1)]),
            Reason::Blockers => r.block(vec![]),
            Reason::Priority
                if r.player == P1 && g.state.rules.turn.step == TurnStep::DeclareBlockers =>
            {
                break;
            }
            Reason::Priority => mtg_witness::temporary::pass(&r),
            other => panic!("ambush {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    Driver::cast(&mut g, CardId(0));
    mtg_witness::equipped::pay_to_priority(&mut g, u64::MAX, vec![]);
    combat(&mut g, &[], &[], no_choice);
    assert_eq!(life(&g, P1), 40.into());
    assert_eq!(g.state.rules.effects.len(), 1);
}

/// CR 616.1f, 615.7: after each application only the effects that still
/// apply are offered again. Seat 1's shield prevents one Bears' 2 damage;
/// Urza's Armor and the rest of the shield then compete for the other's,
/// and the choice is taken with the applied shield prefix pending.
#[test]
fn replacement_prevention_reoffers_applicable_effects_after_each_choice() {
    let mut g = game(&[
        ("Shieldmate's Blessing", 0, hand(P0)),
        ("Plains", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Grizzly Bears", 0, bf()),
        ("Urza's Armor", 1, bf()),
    ]);
    cast(&mut g, 0, vec![TargetEntity::Player { player: P1 }]);
    let armor = object(&g, 4);
    let mut seen = vec![];
    let choices = combat(&mut g, &[2, 3], &[], |g, d| {
        seen.push(d.candidates.len());
        let tx = g.damage_transaction().unwrap();
        let from = |c: &Candidate<Rules>, card: u32, armor_effect: bool| {
            matches!(c, Candidate::DamageEffect { effect, damage }
                if tx.entries[damage.0 as usize].source.object.card == CardId(card)
                    && matches!(effect, EffectKey::StaticAbility { source, .. } if *source == armor) == armor_effect)
        };
        if tx.applications.is_empty() {
            d.candidates.iter().position(|c| from(c, 2, false)).unwrap()
        } else {
            d.candidates.iter().position(|c| from(c, 3, true)).unwrap()
        }
    });
    assert_eq!((choices, seen), (2, vec![4, 2]));
    assert_eq!(life(&g, P1), 40.into());
    assert!(
        g.state.rules.effects.is_empty(),
        "2 + 1 used the whole shield"
    );
}

/// CR 615.12, 702.16e: protection is a prevention effect, so it prevents
/// nothing while Frenzied Baloth says combat damage can't be prevented;
/// without Baloth the green Bears' damage to the Zombie is prevented.
#[test]
fn replacement_prevention_unpreventable_damage_ignores_protection() {
    for baloth in [false, true] {
        let mut cards = vec![("Grizzly Bears", 0, bf()), ("Vodalian Zombie", 1, bf())];
        if baloth {
            cards.push(("Frenzied Baloth", 0, bf()));
        }
        let mut g = game(&cards);
        combat(&mut g, &[0], &[(1, 0)], no_choice);
        if baloth {
            assert_eq!(zone(&g, 1), Zone::Graveyard { owner: P1 });
        } else {
            assert_eq!(damage(&g, 1), Quantity::zero());
        }
        // The Zombie's own lethal damage to the Bears is never prevented.
        assert_eq!(zone(&g, 0), Zone::Graveyard { owner: P0 });
    }
}

/// CR 712.8d, 800.4a: Black Panther (King T'Challa's back face, cast
/// with flash) prevents the damage dealt to it as it blocks; its owner then
/// concedes while another recipient's choice in the same simultaneous
/// damage is pending. The applied prefix still names the ability of the
/// face its holder had up when it applied (CR 712.18), and recovers.
#[test]
fn replacement_prevention_departed_holder_prefix_survives_concession() {
    let mut g = seated(
        3,
        &[
            ("King T'Challa", 1, hand(P1)),
            ("Yargle and Multani", 0, bf()),
            ("Grizzly Bears", 0, bf()),
            ("Shieldmate's Blessing", 0, hand(P0)),
            ("Plains", 0, bf()),
            ("Plains", 1, bf()),
            ("Island", 1, bf()),
            ("Forest", 1, bf()),
            ("Forest", 1, bf()),
            ("Forest", 1, bf()),
            ("Forest", 1, bf()),
            ("Urza's Armor", 2, bf()),
        ],
    );
    cast(&mut g, 3, vec![TargetEntity::Player { player: P2 }]);
    let (mut flashed, mut conceded, mut after) = (false, false, 0);
    for _ in 0..120 {
        let r = Driver::request(&mut g, u64::MAX);
        let answer = match &r.reason {
            Reason::Priority if r.player == P1 && !flashed => {
                flashed = true;
                let card = object(&g, 0);
                let answer = r
                    .pick_one(&Candidate::Action {
                        action: Action::CastBackFace { card },
                    })
                    .unwrap();
                g.respond(r.player, r.id, answer).unwrap();
                Driver::pay(&mut g, &[5, 6, 7, 8, 9, 10]);
                continue;
            }
            Reason::Attackers => r.attack(vec![(object(&g, 1), P1), (object(&g, 2), P2)]),
            Reason::Blockers if r.player == P1 => r.block(vec![(object(&g, 0), object(&g, 1))]),
            Reason::Blockers => r.block(vec![]),
            Reason::DamageEffect { .. } if !conceded => {
                assert_eq!(r.player, P2);
                let panther = object(&g, 0);
                let tx = g.damage_transaction().unwrap();
                assert!(tx.applications.iter().any(|a| matches!(a.effect,
                    EffectKey::StaticAbility { source, .. } if source == panther)));
                g.concede(P1).unwrap();
                conceded = true;
                continue;
            }
            Reason::DamageEffect { .. } => {
                // The departed holder's applied prefix is still pending.
                assert!(!g.damage_transaction().unwrap().applications.is_empty());
                checked(&g);
                after += 1;
                r.select(r.candidates[0].clone())
            }
            Reason::Priority if g.state.rules.turn.step == TurnStep::PostcombatMain => break,
            Reason::Priority => mtg_witness::temporary::pass(&r),
            other => panic!("concession {other:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    assert!(flashed && conceded && after == 1 && !g.is_live(P1));
    // CR 800.4a: the conceding player's Black Panther has left the game.
    assert!(
        g.state
            .rules
            .objects
            .get(&CardId(0))
            .is_none_or(|o| o.zone == Zone::Exile)
    );
    assert_eq!(life(&g, P2), 40.into());
    checked(&g);
}
