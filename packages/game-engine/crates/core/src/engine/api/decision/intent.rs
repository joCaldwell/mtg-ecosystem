//! Test intents resolve against the exact decision being answered. A missing
//! selection becomes an out-of-range index so negative witnesses reach the
//! same validator as a client submitting an invalid answer.
use super::*;
use crate::engine::*;

impl<S: Space> Decision<S> {
    pub fn test_index(&self, row: Candidate<S>) -> u32 {
        self.index(&row).unwrap_or(u32::MAX)
    }
    pub fn select(&self, row: Candidate<S>) -> Answer {
        Answer::Pick {
            picks: vec![vec![self.test_index(row)]],
        }
    }
    pub fn pass(&self) -> Answer {
        self.select(Candidate::Action {
            action: Action::Pass,
        })
    }
    pub fn pick_object(&self, object: S::Object) -> Answer {
        self.select(Candidate::Object { object })
    }
    pub fn pick_objects(&self, objects: Vec<S::Object>) -> Answer {
        Answer::Pick {
            picks: vec![
                objects
                    .into_iter()
                    .map(|object| self.test_index(Candidate::Object { object }))
                    .collect(),
            ],
        }
    }
    pub fn order_objects(&self, objects: Vec<S::Object>) -> Answer {
        Answer::Order {
            order: objects
                .into_iter()
                .map(|object| self.test_index(Candidate::Object { object }))
                .collect(),
        }
    }
    pub fn targets(&self, objects: Vec<S::Object>) -> Answer {
        Answer::Pick {
            picks: objects
                .into_iter()
                .map(|object| vec![self.test_index(Candidate::Object { object })])
                .collect(),
        }
    }
    pub fn player_targets(&self, players: Vec<PlayerId>) -> Answer {
        Answer::Pick {
            picks: players
                .into_iter()
                .map(|player| vec![self.test_index(Candidate::Player { player })])
                .collect(),
        }
    }
    pub fn activate_mana(&self, object: S::Object) -> Answer {
        let row = Candidate::Action {
            action: Action::ActivateMana { object },
        };
        if matches!(self.shape, Shape::Pay { .. }) {
            Answer::Pay {
                pay: PayAnswer::Activate {
                    source: self.test_index(row),
                },
            }
        } else {
            self.select(row)
        }
    }
    pub fn attack(&self, attacks: Vec<(S::Object, PlayerId)>) -> Answer {
        self.assign(
            attacks
                .into_iter()
                .map(|(source, player)| (source, Candidate::Player { player })),
        )
    }
    pub fn block(&self, blocks: Vec<(S::Object, S::Object)>) -> Answer {
        self.assign(
            blocks
                .into_iter()
                .map(|(source, object)| (source, Candidate::Object { object })),
        )
    }
    fn assign(&self, choices: impl Iterator<Item = (S::Object, Candidate<S>)>) -> Answer {
        let Shape::Pick { slots, .. } = &self.shape else {
            return Answer::Pick { picks: vec![] };
        };
        let mut picks = vec![vec![]; slots.len()];
        for (object, row) in choices {
            let subject = self.test_index(Candidate::Object { object });
            let Some(slot) = slots.iter().position(|s| s.subject == Some(subject)) else {
                return Answer::Pick {
                    picks: vec![vec![u32::MAX]],
                };
            };
            picks[slot].push(self.test_index(row));
        }
        Answer::Pick { picks }
    }
    pub fn objects_offered(&self) -> Vec<S::Object> {
        self.candidates
            .iter()
            .filter_map(|c| match c {
                Candidate::Object { object } => Some(object.clone()),
                _ => None,
            })
            .collect()
    }
    pub fn subjects(&self) -> Vec<S::Object> {
        let Shape::Pick { slots, .. } = &self.shape else {
            panic!("expected pick")
        };
        slots
            .iter()
            .filter_map(|s| s.subject)
            .map(|i| match &self.candidates[i as usize] {
                Candidate::Object { object } => object.clone(),
                _ => panic!("expected object subject"),
            })
            .collect()
    }
    pub fn required_count(&self) -> usize {
        match &self.shape {
            Shape::Order { min, .. } => *min as usize,
            Shape::Pick { slots, .. } => slots[0].min as usize,
            _ => panic!("expected selection"),
        }
    }
}
impl Decision<Rules> {
    pub fn cast(&self, card: CardId) -> Answer {
        let index = self.candidates.iter().position(
            |c| matches!(c,Candidate::Action{action:Action::Cast{card:id}} if id.card==card),
        );
        Answer::Pick {
            picks: vec![vec![index.map_or(u32::MAX, |i| i as u32)]],
        }
    }
    pub fn land(&self, card: CardId) -> Answer {
        let index = self.candidates.iter().position(
            |c| matches!(c,Candidate::Action{action:Action::PlayLand{object}} if object.card==card),
        );
        Answer::Pick {
            picks: vec![vec![index.map_or(u32::MAX, |i| i as u32)]],
        }
    }
}

impl<S: Space> Decision<S> {
    pub fn damage_to(&self, assignments: Vec<(S::Object, Quantity)>) -> Answer {
        let Shape::Distribute { buckets, .. } = &self.shape else {
            return Answer::Distribute { amounts: vec![] };
        };
        let mut amounts = vec![None; buckets.len()];
        for (object, value) in assignments {
            let i = self.test_index(Candidate::Object { object });
            let Some(bucket) = buckets.iter().position(|b| b.candidate == i) else {
                return Answer::Distribute { amounts: vec![] };
            };
            if amounts[bucket].replace(value).is_some() {
                return Answer::Distribute { amounts: vec![] };
            }
        }
        Answer::Distribute {
            amounts: amounts.into_iter().flatten().collect(),
        }
    }
}
impl Decision<Rules> {
    pub fn entity_targets(&self, targets: Vec<TargetEntity>) -> Answer {
        Answer::Pick {
            picks: targets
                .into_iter()
                .map(|e| {
                    vec![self.test_index(match e {
                        TargetEntity::Object { object } => Candidate::Object { object },
                        TargetEntity::Player { player } => Candidate::Player { player },
                    })]
                })
                .collect(),
        }
    }
}

impl<S: Space> Decision<S> {
    pub fn allowed_objects(&self) -> Vec<S::Object> {
        let Shape::Pick { slots, .. } = &self.shape else {
            panic!("expected pick")
        };
        self.candidates
            .iter()
            .enumerate()
            .filter(|(i, _)| slots.iter().any(|s| s.allowed.contains(&(*i as u32))))
            .filter_map(|(_, c)| match c {
                Candidate::Object { object } => Some(object.clone()),
                _ => None,
            })
            .collect()
    }
}
