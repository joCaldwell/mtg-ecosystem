//! Categorical prefixes of one permitted decision. A pointer is offered only
//! if its prefix extends to a structurally valid answer.
use mtg_contracts::Quantity;
use mtg_view::*;
pub const STOP: usize = 128;
pub const FINISH_UNDO: usize = 129;
pub const FINISH_KEEP: usize = 130;
pub const MAXIMUM: usize = 63;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    pub pointer: [u8; 131],
    pub number: [u8; 64],
    pub numeric: bool,
    pub slot: usize,
    pub picked: usize,
    pub remaining: i32,
}
impl Choices {
    fn new(slot: usize, picked: usize) -> Self {
        Self {
            pointer: [0; 131],
            number: [0; 64],
            numeric: false,
            slot,
            picked,
            remaining: 0,
        }
    }
}
pub enum Step {
    Choose(Box<Choices>),
    Answer(Answer),
}
pub struct Composer;
impl Composer {
    /// The bound comes exclusively from the acting player's view and catalog.
    pub fn compose(
        d: &Decision<Viewer>,
        actions: &[usize],
        bound: Option<&Quantity>,
    ) -> Result<Step, String> {
        if d.candidates.len() > STOP {
            return Err("decision exceeds candidate capacity".into());
        }
        let mut actions = actions.iter().copied();
        let mut ask = |choices: Choices| -> Result<Result<usize, Step>, String> {
            let mask: &[u8] = if choices.numeric {
                &choices.number
            } else {
                &choices.pointer
            };
            if !mask.contains(&1) {
                return Err("decision has no encodable completion".into());
            }
            match actions.next() {
                Some(a) if mask.get(a) == Some(&1) => Ok(Ok(a)),
                Some(_) => Err("masked micro-action".into()),
                None => Ok(Err(Step::Choose(Box::new(choices)))),
            }
        };
        macro_rules! select {
            ($choices:expr) => {
                match ask($choices)? {
                    Ok(a) => a,
                    Err(step) => return Ok(step),
                }
            };
        }
        let answer = match &d.shape {
            Shape::Pick { slots, rules } => {
                let mut picks = vec![vec![]; slots.len()];
                // CR 701.23a: a search with no matching card still looks at
                // the library; the observation shows it and stop finds nothing.
                if matches!(d.reason, Reason::SearchLibrary) && slots.iter().all(|s| s.max == 0) {
                    let mut choices = Choices::new(0, 0);
                    choices.pointer[STOP] = 1;
                    select!(choices);
                }
                for (i, slot) in slots.iter().enumerate() {
                    while picks[i].len() < slot.max as usize {
                        let mut choices = Choices::new(i, picks.iter().map(Vec::len).sum());
                        if picks[i].len() >= slot.min as usize
                            && feasible(slots, rules, &mut picks, i + 1)
                        {
                            choices.pointer[STOP] = 1;
                        }
                        for &id in &slot.allowed {
                            let idx = pointer(d, id)?;
                            if permits(rules, slot, &picks, i, id) {
                                picks[i].push(id);
                                choices.pointer[idx] = feasible(slots, rules, &mut picks, i) as u8;
                                picks[i].pop();
                            }
                        }
                        let a = select!(choices);
                        if a == STOP {
                            break;
                        }
                        picks[i].push(a as u32);
                    }
                }
                Answer::Pick { picks }
            }
            Shape::Order { items, min, max } => {
                let mut order = vec![];
                while order.len() < *max as usize {
                    let mut c = Choices::new(0, order.len());
                    for &id in items {
                        if !order.contains(&id) {
                            c.pointer[pointer(d, id)?] = 1;
                        }
                    }
                    c.pointer[STOP] = (order.len() >= *min as usize) as u8;
                    let a = select!(c);
                    if a == STOP {
                        break;
                    }
                    order.push(a as u32);
                }
                Answer::Order { order }
            }
            Shape::Number { min, max } => {
                // An unbounded rules decision and unknown public supply have no
                // finite rules maximum. Use the numeric tensor's representable ceiling.
                // The supply estimate never removes the rules minimum (CR 601.2b-c):
                // a target filter can require more X than visible mana.
                let upper = max
                    .as_ref()
                    .into_iter()
                    .chain(bound)
                    .min()
                    .cloned()
                    .unwrap_or_else(|| Quantity::from(i64::from(i32::MAX)))
                    .max(min.clone());
                let mut c = Choices::new(0, 0);
                numbers(&mut c, min, &upper);
                let action = select!(c);
                Answer::Number {
                    value: decode_number(action, &upper),
                }
            }
            Shape::CombatDistribute { groups, .. } => combat_allocation_to_blockers(groups),
            Shape::Distribute {
                total,
                buckets,
                gates,
            } => {
                let mut amounts = vec![];
                let mut remaining = total.clone();
                for i in 0..buckets.len() {
                    if i + 1 == buckets.len() {
                        amounts.push(remaining);
                        break;
                    }
                    let mut c = Choices::new(i, i);
                    c.remaining = crate::int(&remaining);
                    let bounds: Vec<_> = buckets
                        .iter()
                        .enumerate()
                        .map(|(j, b)| match amounts.get(j) {
                            Some(q) => (q.clone(), q.clone()),
                            None => (b.min.clone(), b.max.clone()),
                        })
                        .collect();
                    let mut upper = None;
                    allocation_ranges(total, gates, bounds, i, &mut |low, high| {
                        numbers(&mut c, &low, &high);
                        if upper.as_ref().is_none_or(|n| &high > n) {
                            upper = Some(high);
                        }
                    })?;
                    let a = select!(c);
                    let value = decode_number(a, &upper.ok_or("no allocation")?);
                    remaining.subtract(&value);
                    amounts.push(value);
                }
                Answer::Distribute { amounts }
            }
            Shape::Pay {
                step: PayStep::Produce { sources },
                ..
            } => {
                let mut c = Choices::new(0, 0);
                for &id in sources {
                    c.pointer[pointer(d, id)?] = 1;
                }
                c.pointer[FINISH_UNDO..].fill(1);
                let a = select!(c);
                let pay = if a >= FINISH_UNDO {
                    PayAnswer::Finish {
                        undo_on_failure: a == FINISH_UNDO,
                    }
                } else {
                    PayAnswer::Activate { source: a as u32 }
                };
                Answer::Pay { pay }
            }
            Shape::Pay {
                step: PayStep::Spend,
                cost,
                pool,
            } => Answer::Pay {
                pay: pool.spend(cost).ok_or("insufficient mana")?,
            },
        };
        if actions.next().is_some() {
            return Err("micro-actions after completion".into());
        }
        d.shape.check(&answer)?;
        Ok(Step::Answer(answer))
    }
}
fn pointer(d: &Decision<Viewer>, id: u32) -> Result<usize, String> {
    d.candidate(id).map(|_| id as usize)
}
fn numbers(c: &mut Choices, min: &Quantity, max: &Quantity) {
    c.numeric = true;
    if min > max {
        return;
    }
    for (i, flag) in c.number[..MAXIMUM].iter_mut().enumerate() {
        let n = Quantity::from(i as i64);
        *flag |= (min <= &n && &n <= max) as u8;
    }
    c.number[MAXIMUM] = 1;
}
fn decode_number(i: usize, max: &Quantity) -> Quantity {
    if i == MAXIMUM {
        max.clone()
    } else {
        (i as i64).into()
    }
}
fn permits(rules: &[JointRule], slot: &Slot, picks: &[Vec<u32>], i: usize, id: u32) -> bool {
    (slot.repeat || !picks[i].contains(&id))
        && rules.iter().all(|r| match r {
            JointRule::Distinct { slots } if slots.contains(&(i as u32)) => !slots
                .iter()
                .any(|&j| picks.get(j as usize).is_none_or(|p| p.contains(&id))),
            JointRule::Total { max, .. } => {
                picks.iter().map(Vec::len).sum::<usize>() < *max as usize
            }
            JointRule::PickedAtMost { candidates, count } => {
                !candidates.contains(&id)
                    || picks.iter().flatten().filter(|x| **x == id).count() < *count as usize
            }
            _ => true,
        })
}
// Stop-first completion search. Prune impossible cardinality/menace prefixes
// before branching; ordinary one-slot picks require no combinatorial enumeration.
fn feasible(slots: &[Slot], rules: &[JointRule], picks: &mut [Vec<u32>], i: usize) -> bool {
    let total: usize = picks.iter().map(Vec::len).sum();
    for r in rules {
        match r {
            JointRule::Total { min, max } => {
                let free: usize = slots[i..]
                    .iter()
                    .zip(&picks[i..])
                    .map(|(s, p)| s.max as usize - p.len())
                    .sum();
                if total > *max as usize || total + free < *min as usize {
                    return false;
                }
            }
            JointRule::PickedAtLeast { candidates, count } => {
                for id in candidates {
                    let n = picks.iter().flatten().filter(|x| *x == id).count();
                    if n == 0 || n >= *count as usize {
                        continue;
                    }
                    let possible: usize = slots[i..]
                        .iter()
                        .zip(&picks[i..])
                        .filter(|(s, _)| s.allowed.contains(id))
                        .map(|(s, p)| {
                            if s.repeat {
                                s.max as usize - p.len()
                            } else {
                                usize::from(!p.contains(id) && p.len() < s.max as usize)
                            }
                        })
                        .sum();
                    if n + possible < *count as usize {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    let Some(s) = slots.get(i) else { return true };
    if picks[i].len() >= s.min as usize && feasible(slots, rules, picks, i + 1) {
        return true;
    }
    if picks[i].len() >= s.max as usize {
        return false;
    }
    for &id in &s.allowed {
        if permits(rules, s, picks, i, id) {
            picks[i].push(id);
            let good = feasible(slots, rules, picks, i);
            picks[i].pop();
            if good {
                return true;
            }
        }
    }
    false
}
type Range = (Quantity, Quantity);
// A gate is a disjunction: zero in its bucket, or all its prerequisites.
// For each feasible conjunction, projecting a bounded integer sum onto a
// single bucket produces a closed interval, even with exact large quantities.
fn allocation_ranges(
    total: &Quantity,
    gates: &[Gate],
    mut bounds: Vec<Range>,
    i: usize,
    emit: &mut impl FnMut(Quantity, Quantity),
) -> Result<(), String> {
    if bounds.iter().any(|(a, b)| a > b) {
        return Ok(());
    }
    if let Some((gate, rest)) = gates.split_first() {
        let at = gate.bucket as usize;
        let (lo, hi) = bounds.get(at).ok_or("invalid gated bucket")?;
        if lo <= &Quantity::zero() {
            let mut off = bounds.clone();
            off[at].1 = Quantity::zero();
            allocation_ranges(total, rest, off, i, emit)?;
        }
        if hi > &Quantity::zero() {
            for (j, needed) in &gate.requires {
                let (min, _) = bounds
                    .get_mut(*j as usize)
                    .ok_or("invalid gate requirement")?;
                if &*min < needed {
                    *min = needed.clone();
                }
            }
            allocation_ranges(total, rest, bounds, i, emit)?;
        }
    } else {
        let mut low = total.clone();
        let mut high = total.clone();
        for (j, (a, b)) in bounds.iter().enumerate() {
            if j != i {
                low.subtract(b);
                high.subtract(a);
            }
        }
        let low = low.max(bounds[i].0.clone());
        let high = high.min(bounds[i].1.clone());
        if low <= high {
            emit(low, high);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_and_forced_last_bucket() {
        let d = Decision {
            id: DecisionToken("test".into()),
            player: PlayerId(0),
            reason: Reason::OptionalEffect,
            candidates: vec![],
            shape: Shape::Distribute {
                total: 100.into(),
                buckets: (0..2)
                    .map(|i| Bucket {
                        candidate: i,
                        min: 0.into(),
                        max: 100.into(),
                    })
                    .collect(),
                gates: vec![Gate {
                    bucket: 1,
                    requires: vec![(0, 3.into())],
                }],
            },
        };
        let Step::Choose(c) = Composer::compose(&d, &[], None).unwrap() else {
            panic!()
        };
        assert_eq!(&c.number[..4], &[0, 0, 0, 1]);
        assert!(
            matches!(Composer::compose(&d,&[63],None).unwrap(), Step::Answer(Answer::Distribute{amounts}) if amounts==vec![100.into(),0.into()])
        );
    }
    /// CR 601.2b-c: a rules minimum above the visible supply estimate is
    /// still the encodable answer.
    #[test]
    fn number_minimum_above_the_supply_bound_stays_encodable() {
        let d = Decision {
            id: DecisionToken("x".into()),
            player: PlayerId(0),
            reason: Reason::AnnounceX,
            candidates: vec![],
            shape: Shape::Number {
                min: 6.into(),
                max: None,
            },
        };
        let bound = Quantity::from(3);
        let Step::Choose(c) = Composer::compose(&d, &[], Some(&bound)).unwrap() else {
            panic!("expected mask")
        };
        assert_eq!(
            c.number
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0)
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            vec![6, MAXIMUM]
        );
        assert!(matches!(
            Composer::compose(&d, &[6], Some(&bound)).unwrap(),
            Step::Answer(Answer::Number { value }) if value == 6.into()
        ));
    }
    #[test]
    fn picks_offer_only_prefixes_extending_to_a_joint_solution() {
        let slot = |allowed: Vec<u32>| Slot {
            role: SlotRole::Choice,
            subject: None,
            allowed,
            min: 0,
            max: 1,
            repeat: false,
        };
        let mut d = Decision {
            id: DecisionToken("slots".into()),
            player: PlayerId(0),
            reason: Reason::Blockers,
            candidates: vec![
                Candidate::Player {
                    player: PlayerId(1),
                },
                Candidate::Player {
                    player: PlayerId(2),
                },
            ],
            shape: Shape::Pick {
                slots: vec![slot(vec![0, 1]), slot(vec![0, 1])],
                rules: vec![
                    JointRule::PickedAtLeast {
                        candidates: vec![0],
                        count: 2,
                    },
                    JointRule::Total { min: 1, max: 2 },
                ],
            },
        };
        let allowed = |d: &Decision<Viewer>, prefix: &[usize]| {
            let Step::Choose(c) = Composer::compose(d, prefix, None).unwrap() else {
                panic!("expected mask")
            };
            c.pointer
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0)
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        assert_eq!(allowed(&d, &[0]), vec![0]);
        assert_eq!(allowed(&d, &[STOP]), vec![1]);
        if let Shape::Pick { rules, .. } = &mut d.shape {
            rules.push(JointRule::Distinct { slots: vec![0, 1] });
        }
        assert_eq!(allowed(&d, &[]), vec![1, STOP]);
        let Shape::Pick { slots, rules } = &mut d.shape else {
            unreachable!()
        };
        slots.truncate(1);
        slots[0].repeat = true;
        slots[0].max = 2;
        rules.pop();
        assert_eq!(allowed(&d, &[0]), vec![0]);
        assert!(matches!(
            Composer::compose(&d, &[0, 0], None).unwrap(),
            Step::Answer(_)
        ));
    }
    /// CR 701.23b, 701.22a: a search with a stated quality may find nothing
    /// (the stop pointer is offered first), and an order of the cards going
    /// back on top must name each of them once before it stops.
    #[test]
    fn library_choices_offer_fail_to_find_and_complete_orders() {
        let objects = |n: usize| {
            (0..n)
                .map(|i| Candidate::Object {
                    object: ObjectHandle(format!("v0-o{i}")),
                })
                .collect::<Vec<_>>()
        };
        let search = Decision {
            id: DecisionToken("search".into()),
            player: PlayerId(0),
            reason: Reason::SearchLibrary,
            candidates: objects(2),
            shape: Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject: None,
                    allowed: vec![0, 1],
                    min: 0,
                    max: 1,
                    repeat: false,
                }],
                rules: vec![],
            },
        };
        let Step::Choose(c) = Composer::compose(&search, &[], None).unwrap() else {
            panic!("expected mask")
        };
        assert_eq!((c.pointer[0], c.pointer[1], c.pointer[STOP]), (1, 1, 1));
        assert!(matches!(
            Composer::compose(&search, &[STOP], None).unwrap(),
            Step::Answer(Answer::Pick { picks }) if picks == vec![Vec::<u32>::new()]
        ));
        assert!(matches!(
            Composer::compose(&search, &[1], None).unwrap(),
            Step::Answer(Answer::Pick { picks }) if picks == vec![vec![1]]
        ));
        // CR 701.23a: a library with no matching card is still inspected;
        // the only pointer is the stop that finds nothing.
        let none = Decision {
            candidates: vec![],
            shape: Shape::Pick {
                slots: vec![Slot {
                    role: SlotRole::Choice,
                    subject: None,
                    allowed: vec![],
                    min: 0,
                    max: 0,
                    repeat: false,
                }],
                rules: vec![],
            },
            ..search.clone()
        };
        let Step::Choose(c) = Composer::compose(&none, &[], None).unwrap() else {
            panic!("expected mask")
        };
        assert_eq!(c.pointer.iter().filter(|p| **p == 1).count(), 1);
        assert_eq!(c.pointer[STOP], 1);
        assert!(matches!(
            Composer::compose(&none, &[STOP], None).unwrap(),
            Step::Answer(Answer::Pick { picks }) if picks == vec![Vec::<u32>::new()]
        ));
        let order = Decision {
            id: DecisionToken("order".into()),
            player: PlayerId(0),
            reason: Reason::OrderLibraryTop,
            candidates: objects(2),
            shape: Shape::Order {
                items: vec![0, 1],
                min: 2,
                max: 2,
            },
        };
        let Step::Choose(c) = Composer::compose(&order, &[1], None).unwrap() else {
            panic!("expected mask")
        };
        assert_eq!((c.pointer[0], c.pointer[1], c.pointer[STOP]), (1, 0, 0));
        assert!(matches!(
            Composer::compose(&order, &[1, 0], None).unwrap(),
            Step::Answer(Answer::Order { order }) if order == vec![1, 0]
        ));
    }
}
