//! Integer encoding of the viewer's published knowledge. Hidden zone access
//! follows CR 400.2 and 401.2 (pinned20260925):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub mod action;
pub mod catalog;
mod funding;
use mtg_contracts::{Catalog, Color, HybridPayment, Quantity};
use mtg_view::*;
use std::collections::HashMap;

/// Fixed number of object, stack, remembered-library and known-hand rows in
/// each observation.
pub const MAX_ENTITIES: usize = 256;

/// A frame can only be exposed if every entity and every candidate has a row.
pub fn fits_layout(view: &PermittedView, decision: &Decision<Viewer>) -> bool {
    view.objects.len()
        + view.stack.len()
        + view.known_library_positions.len()
        + view.known_searched_hand.len()
        + view.revealed_library_cards.len()
        <= MAX_ENTITIES
        && decision.candidates.len() <= action::STOP
        && view.players.len() <= 4
}

pub fn int(q: &Quantity) -> i32 {
    if let Some(n) = q.to_u32() {
        return i32::try_from(n).unwrap_or(i32::MAX);
    }
    if q.is_negative() {
        q.to_string().parse().unwrap_or(i32::MIN)
    } else {
        i32::MAX
    }
}
pub fn color(c: Color) -> usize {
    match c {
        Color::W => 0,
        Color::U => 1,
        Color::B => 2,
        Color::R => 3,
        Color::G => 4,
        Color::C => 5,
    }
}
fn card(c: &Catalog, id: &str) -> Result<i32, String> {
    if id.is_empty() {
        return Ok(0);
    }
    c.definitions
        .binary_search_by(|d| d.oracle_id.as_str().cmp(id))
        .map(|i| i as i32 + 1)
        .map_err(|_| format!("missing visible catalog identity {id}"))
}
/// A finite payment ceiling is justified only for a fully modeled AnnounceX.
pub fn number_bound(c: &catalog::ObservationCatalog, o: &Observation) -> Option<Quantity> {
    if !o
        .decision
        .as_ref()
        .is_some_and(|d| matches!(d.reason, Reason::AnnounceX))
    {
        return None;
    }
    let v = &o.view;
    if c.has_visible_reducer(v)
        || v.objects
            .iter()
            .any(|o| o.zone == Zone::Stack && c.reduces_cost(&o.oracle_id))
    {
        return None;
    }
    let (pool, complete) = funding::intrinsic_supply(c, v);
    complete.then(|| {
        let mut total = Quantity::zero();
        for amount in pool.values() {
            total.add(amount);
        }
        total
    })
}
#[derive(Clone, PartialEq, Eq)]
pub struct Row {
    pub globals: [i32; 48],
    pub players: [[i32; 24]; 4],
    pub ent_int: [[i32; 10]; MAX_ENTITIES],
    pub ent_num: [[i32; 24]; MAX_ENTITIES],
    pub ent_mask: [u8; MAX_ENTITIES],
    pub cand: [[i32; 6]; 128],
    pub ptr_mask: [u8; 131],
    pub num_mask: [u8; 64],
    pub head: u8,
    pub seat: u8,
    pub reward: [i32; 4],
    pub done: u8,
    pub info: [i32; 8],
}
impl Default for Row {
    fn default() -> Self {
        Self {
            globals: [0; 48],
            players: [[0; 24]; 4],
            ent_int: [[-1; 10]; MAX_ENTITIES],
            ent_num: [[0; 24]; MAX_ENTITIES],
            ent_mask: [0; MAX_ENTITIES],
            cand: [[-1; 6]; 128],
            ptr_mask: [0; 131],
            num_mask: [0; 64],
            head: 0,
            seat: 0,
            reward: [0; 4],
            done: 0,
            info: [0; 8],
        }
    }
}
fn zone(z: Zone) -> i32 {
    match z {
        Zone::Battlefield => 0,
        Zone::Hand { .. } => 1,
        Zone::Graveyard { .. } => 2,
        Zone::Library { .. } => 3,
        Zone::Command { .. } => 4,
        Zone::Exile => 5,
        Zone::Stack => 6,
    }
}
pub fn encode(
    c: &catalog::ObservationCatalog,
    o: &Observation,
    micro: usize,
    choices: &action::Choices,
    mask_unfundable: bool,
) -> Result<Row, String> {
    let v = &o.view;
    let d = o.decision.as_ref().ok_or("expected viewer decision")?;
    let relative = |p: PlayerId| i32::from((p.0 + 4 - v.viewer.0) % 4);
    if !fits_layout(v, d) {
        return Err("permitted view exceeds tensor capacity".into());
    }
    let funding = funding::funding(c, o, d);
    let mut r = Row {
        head: choices.numeric as u8,
        seat: v.viewer.0,
        ptr_mask: choices.pointer,
        num_mask: choices.number,
        ..Default::default()
    };
    let g = &mut r.globals;
    g[..12].copy_from_slice(&[
        v.turn.number.min(i32::MAX as u64) as i32,
        v.turn.step as i32,
        v.active.map_or(-1, relative),
        v.priority.map_or(-1, relative),
        v.stack.len() as i32,
        v.live_turn_order.len() as i32,
        match d.shape {
            Shape::Pick { .. } => 0,
            Shape::Order { .. } => 1,
            Shape::Distribute { .. } => 2,
            Shape::Number { .. } => 3,
            Shape::Pay { .. } => 4,
        },
        i32::from(d.reason.code()),
        micro as i32,
        choices.slot as i32,
        choices.picked as i32,
        choices.remaining,
    ]);
    if let Shape::Pay { cost, pool, .. } = &d.shape {
        for &c in &cost.colored {
            g[12 + color(c)] += 1;
        }
        g[18] = int(&cost.generic);
        for (c, q) in pool.available() {
            g[19 + color(c)] = int(&q);
        }
    } else if let Some(p) = v.players.iter().find(|p| p.player == v.viewer) {
        for (&c, q) in &p.mana {
            g[19 + color(c)] = int(q);
        }
    }
    g[26] = v.setup.as_ref().map_or(0, |s| match s.phase {
        VisibleSetupPhase::Declare => 1,
        VisibleSetupPhase::Bottom => 2,
    });
    for p in &v.players {
        let row = &mut r.players[relative(p.player) as usize];
        row[..4].copy_from_slice(&[
            int(&p.life),
            p.hand_count as i32,
            p.library_count as i32,
            p.library_shuffles.min(i32::MAX as u64) as i32,
        ]);
        for (&c, q) in &p.mana {
            row[4 + color(c)] = int(q);
        }
        row[10] = p.status as i32;
        row[11] = p.life_frozen_until_next_turn as i32;
        for cmd in &v.commanders {
            let src = relative(cmd.history.owner) as usize;
            if let Some(q) = cmd.history.command_casts.get(&p.player) {
                row[12 + src] = row[12 + src].saturating_add(int(q));
            }
            if let Some(q) = cmd.history.combat_damage.get(&p.player) {
                row[16 + src] = row[16 + src].saturating_add(int(q));
            }
        }
    }
    let objects: HashMap<_, _> = v
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.handle.0.as_str(), i as i32))
        .collect();
    let object = |h: &ObjectHandle| objects.get(h.0.as_str()).copied().unwrap_or(-1);
    for (i, o) in v.objects.iter().enumerate() {
        r.ent_mask[i] = 1;
        let row = &mut r.ent_int[i];
        row[..6].copy_from_slice(&[
            card(c, &o.oracle_id)?,
            zone(o.zone),
            relative(o.owner),
            relative(o.controller),
            o.attached_to.as_ref().map_or(-1, object),
            o.exiled_by.as_ref().map_or(-1, object),
        ]);
        let n = &mut r.ent_num[i];
        n[..6].copy_from_slice(&[
            o.tapped as i32,
            o.attack_ready as i32,
            o.entered_this_turn as i32,
            o.phased_out as i32,
            o.face_down as i32,
            o.token.is_some() as i32,
        ]);
        if let Some(ch) = &o.characteristics {
            n[6] = ch.power.as_ref().map_or(0, int);
            n[7] = ch.toughness.as_ref().map_or(0, int);
        }
        n[8] = int(&o.marked_damage);
        n[9] = int(&o.plus_one_counters);
        // CR 122.1: -1/-1 counters, then every other kind together.
        for counter in &o.counters {
            let column = if counter.kind == mtg_contracts::kir::CounterKind::MinusOneMinusOne {
                20
            } else {
                21
            };
            n[column] = n[column].saturating_add(int(&counter.count));
        }
        for k in &o.keywords {
            let bit = *k as usize;
            n[11 + bit / 32] |= 1 << (bit % 32);
        }
        for k in &o.restrictions {
            n[13] |= 1 << (*k as u32);
        }
        n[10] = i32::try_from(o.granted_mana.len()).unwrap_or(i32::MAX);
        n[15] = -1;
        n[18] = -1;
    }
    for a in &v.combat {
        let i = object(&a.attacker);
        if i >= 0 {
            r.ent_int[i as usize][7] = relative(a.defender);
        }
        for b in &a.blockers {
            let j = object(b);
            if j >= 0 {
                r.ent_int[j as usize][6] = i;
            }
        }
    }
    let base = v.objects.len();
    let stack: HashMap<_, _> = v
        .stack
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.handle.as_ref().map(|h| (h.0.as_str(), (base + i) as i32)))
        .collect();
    for (j, s) in v.stack.iter().enumerate() {
        let i = base + j;
        r.ent_mask[i] = 1;
        let row = &mut r.ent_int[i];
        let source = s
            .spell
            .as_ref()
            .and_then(|h| objects.get(h.0.as_str()))
            .map(|&x| &v.objects[x as usize]);
        let id = source
            .map(|o| o.oracle_id.as_str())
            .or_else(|| s.ability.as_ref().map(|a| a.oracle_id.as_str()))
            .or_else(|| s.triggered.as_ref().map(|t| t.oracle_id.as_str()))
            .unwrap_or("");
        row[..4].copy_from_slice(&[
            card(c, id)?,
            6,
            source.map_or(-1, |o| relative(o.owner)),
            relative(s.controller),
        ]);
        let targets = s
            .targets
            .iter()
            .map(|t| t.current.as_ref().map_or(-1, object))
            .chain(s.entity_targets.iter().map(|t| match t {
                VisibleEntityTarget::Object { object: t } => t.current.as_ref().map_or(-1, object),
                _ => -1,
            }))
            .chain(s.stack_targets.iter().map(|t| {
                t.current
                    .as_ref()
                    .and_then(|h| stack.get(h.0.as_str()).copied())
                    .unwrap_or(-1)
            }));
        for (field, value) in row[8..].iter_mut().zip(targets) {
            *field = value
        }
        let n = &mut r.ent_num[i];
        n[14] = s.x.as_ref().map_or(0, int);
        n[15] = -1;
        n[16] = s.cast_complete as i32;
        n[17] = s.spell_copy as i32;
        n[18] = -1;
        // CR 702.34a: a flashback spell is exiled as it leaves the stack.
        n[19] = s.flashback as i32;
    }
    for (j, k) in v.known_library_positions.iter().enumerate() {
        let i = base + v.stack.len() + j;
        r.ent_mask[i] = 1;
        let p = relative(k.library_owner);
        r.ent_int[i][..4].copy_from_slice(&[card(c, &k.oracle_id)?, 3, p, p]);
        r.ent_num[i][15] = -1;
        r.ent_num[i][18] = k.position as i32;
    }
    // CR 701.23e: each publicly known copy of a revealed card in a hand, by
    // identity only; it has no handle, position or incarnation.
    let base = base + v.stack.len() + v.known_library_positions.len();
    for (j, (owner, oracle_id)) in v.known_searched_hand.iter().enumerate() {
        let i = base + j;
        r.ent_mask[i] = 1;
        let p = relative(*owner);
        r.ent_int[i][..4].copy_from_slice(&[card(c, oracle_id)?, 1, p, p]);
        r.ent_num[i][15] = -1;
        r.ent_num[i][18] = -1;
    }
    // CR 701.20a: revealed library cards disclose an identity and owner
    // without a private library position or an addressable object row.
    let base = base + v.known_searched_hand.len();
    for (j, (owner, oracle_id)) in v.revealed_library_cards.iter().enumerate() {
        let i = base + j;
        r.ent_mask[i] = 1;
        let p = relative(*owner);
        r.ent_int[i][..4].copy_from_slice(&[card(c, oracle_id)?, 3, p, p]);
        r.ent_num[i][15] = -1;
        r.ent_num[i][18] = -1;
    }
    for (i, k) in d.candidates.iter().enumerate() {
        let (kind, entity, seat, a, b) = match k {
            Candidate::Object { object: h } => (0, object(h), -1, 0, 0),
            Candidate::Stack { stack: h } => {
                (15, stack.get(h.0.as_str()).copied().unwrap_or(-1), -1, 0, 0)
            }
            Candidate::Player { player } => (1, -1, relative(*player), 0, 0),
            Candidate::Trigger { .. } => (2, -1, -1, 0, 0),
            Candidate::DamageEffect { .. } => (3, -1, -1, 0, 0),
            Candidate::Color { color: c } => (4, -1, -1, color(*c) as i32, 0),
            Candidate::ManaBundle { symbols } => {
                let mut counts = [0i32; 6];
                for symbol in symbols {
                    counts[color(*symbol)] += 1;
                }
                let packed = counts
                    .iter()
                    .enumerate()
                    .map(|(index, count)| count << (3 * index))
                    .sum();
                (16, -1, -1, packed, 0)
            }
            Candidate::ManaAbility { ability } => match ability {
                ManaAbilityIdentity::Printed { index } => (5, -1, -1, 1, *index as i32),
                ManaAbilityIdentity::Granted { index } => (5, -1, -1, 2, *index as i32),
                ManaAbilityIdentity::Intrinsic { color: c } => (5, -1, -1, 0, color(*c) as i32),
            },
            Candidate::AdditionalCost { mana, .. } => {
                (6, -1, -1, int(&mana.generic), mana.colored.len() as i32)
            }
            Candidate::HybridPayment { payment } => (
                7,
                -1,
                -1,
                match payment {
                    HybridPayment::Mana(c) => color(*c) as i32,
                    HybridPayment::PayTwoLife => 6,
                },
                0,
            ),
            Candidate::Flag { flag } => (8, -1, -1, *flag as i32, 0),
            Candidate::Modes { mask, .. } => (14, -1, -1, i32::from(*mask), 0),
            Candidate::Action { action } => match action {
                Action::Pass => (9, -1, -1, 0, 0),
                Action::PlayLand { object: h } => (10, object(h), -1, 0, 0),
                Action::Cast { card: h } => (11, object(h), -1, 0, 0),
                Action::PlayLandBackFace { object: h } => (10, object(h), -1, 1, 0),
                Action::CastBackFace { card: h } => (11, object(h), -1, 1, 0),
                Action::CastFaceDown { card: h } => (11, object(h), -1, 2, 0),
                Action::TurnFaceUp { object: h } => (16, object(h), -1, 0, 0),
                Action::ActivateMana { object: h } => (12, object(h), -1, 0, 0),
                Action::ActivateAbility { object: h, ability } => {
                    (13, object(h), -1, *ability as i32, 0)
                }
            },
        };
        r.cand[i] = [kind, entity, seat, a, b, i32::from(funding[i])];
        if mask_unfundable && funding[i] == 0 {
            r.ptr_mask[i] = 0;
        }
    }
    if mask_unfundable && !choices.numeric && !r.ptr_mask.contains(&1) {
        r.ptr_mask = choices.pointer;
    }
    Ok(r)
}
