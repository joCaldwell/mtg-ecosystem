//! An accepted index is interpreted in the decision that issued it. The
//! journal keeps only the answer. The pending rule step also retains its
//! candidate table so a queued player departure cannot silently retarget it.
use super::*;
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingInput {
    pub input: AcceptedInput,
    pub decision: Decision<Rules>,
}
impl std::ops::Deref for PendingInput {
    type Target = AcceptedInput;
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<S: Space> Decision<S> {
    pub fn index(&self, candidate: &Candidate<S>) -> Option<u32> {
        self.candidates
            .iter()
            .position(|row| row == candidate)
            .and_then(|i| i.try_into().ok())
    }
    pub fn candidate(&self, index: u32) -> Result<&Candidate<S>, String> {
        self.candidates
            .get(index as usize)
            .ok_or_else(|| "index outside decision candidates".into())
    }
    pub fn pick_one(&self, candidate: &Candidate<S>) -> Result<Answer, String> {
        let index = self.index(candidate).ok_or("candidate not offered")?;
        let answer = Answer::Pick {
            picks: vec![vec![index]],
        };
        self.shape.check(&answer)?;
        Ok(answer)
    }
}

impl PendingInput {
    fn rows(&self) -> Result<Vec<&Candidate<Rules>>, String> {
        match &self.answer {
            Answer::Pick { picks } => picks
                .iter()
                .flatten()
                .map(|i| self.decision.candidate(*i))
                .collect(),
            Answer::Order { order } => order.iter().map(|i| self.decision.candidate(*i)).collect(),
            _ => Err("expected a candidate selection".into()),
        }
    }
    pub(in crate::engine) fn one(&self) -> Result<&Candidate<Rules>, String> {
        let Answer::Pick { picks } = &self.answer else {
            return Err("expected a pick".into());
        };
        let [slot] = picks.as_slice() else {
            return Err("expected one slot".into());
        };
        let [index] = slot.as_slice() else {
            return Err("expected one candidate".into());
        };
        self.decision.candidate(*index)
    }
    pub(in crate::engine) fn flag(&self, yes: Flag) -> Result<bool, String> {
        let Candidate::Flag { flag } = self.one()? else {
            return Err("expected a flag".into());
        };
        Ok(*flag == yes)
    }
    pub(in crate::engine) fn objects(&self) -> Result<Vec<ObjectId>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::Object { object } => Ok(*object),
                _ => Err("expected an object".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn object(&self) -> Result<ObjectId, String> {
        let Candidate::Object { object } = self.one()? else {
            return Err("expected an object".into());
        };
        Ok(*object)
    }
    pub(in crate::engine) fn players(&self) -> Result<Vec<PlayerId>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::Player { player } => Ok(*player),
                _ => Err("expected a player".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn entities(&self) -> Result<Vec<TargetEntity>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::Player { player } => Ok(TargetEntity::Player { player: *player }),
                Candidate::Object { object } => Ok(TargetEntity::Object { object: *object }),
                _ => Err("expected a damage target".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn triggers(&self) -> Result<Vec<TriggerId>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::Trigger { trigger } => Ok(*trigger),
                _ => Err("expected a trigger".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn hybrid_colors(&self) -> Result<Vec<HybridPayment>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::HybridPayment { payment } => Ok(*payment),
                _ => Err("expected hybrid payment".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn additional_costs(&self) -> Result<Vec<String>, String> {
        self.rows()?
            .into_iter()
            .map(|c| match c {
                Candidate::AdditionalCost { id, .. } => Ok(id.clone()),
                _ => Err("expected additional cost".into()),
            })
            .collect()
    }
    pub(in crate::engine) fn action(&self) -> Result<Action<Rules>, String> {
        let Candidate::Action { action } = self.one()? else {
            return Err("expected priority action".into());
        };
        Ok(action.clone())
    }
    pub(in crate::engine) fn mana_source(&self, index: u32) -> Result<ObjectId, String> {
        let Candidate::Action {
            action: Action::ActivateMana { object },
        } = self.decision.candidate(index)?
        else {
            return Err("expected a mana source".into());
        };
        Ok(*object)
    }
    pub(in crate::engine) fn declarations(
        &self,
    ) -> Result<Vec<(ObjectId, &Candidate<Rules>)>, String> {
        let (Shape::Pick { slots, .. }, Answer::Pick { picks }) =
            (&self.decision.shape, &self.answer)
        else {
            return Err("expected combat declarations".into());
        };
        let mut result = vec![];
        for (slot, picked) in slots.iter().zip(picks) {
            if let [index] = picked.as_slice() {
                let Candidate::Object { object } = self
                    .decision
                    .candidate(slot.subject.ok_or("missing combat subject")?)?
                else {
                    return Err("expected combat creature".into());
                };
                result.push((*object, self.decision.candidate(*index)?));
            }
        }
        Ok(result)
    }
    pub(in crate::engine) fn allocations(
        &self,
    ) -> Result<Vec<(&Candidate<Rules>, Quantity)>, String> {
        let (Shape::Distribute { buckets, .. }, Answer::Distribute { amounts }) =
            (&self.decision.shape, &self.answer)
        else {
            return Err("expected damage allocation".into());
        };
        buckets
            .iter()
            .zip(amounts)
            .map(|(b, n)| Ok((self.decision.candidate(b.candidate)?, n.clone())))
            .collect()
    }
}

impl RulesTxn {
    pub(in crate::engine) fn validate_pending(&self, pending: &PendingInput) -> Result<(), String> {
        if let Frame::Stabilize { resume, .. } = &self.state.rules.frame {
            let mut validation = self.fork();
            validation.rules_mut().frame = self.saved_frame(*resume)?.clone();
            return validation.validate_pending(pending);
        }
        let current = self
            .frame_decision()?
            .ok_or("no decision for pending answer")?;
        if current.player != pending.player || pending.player != pending.decision.player {
            return Err("pending decision chooser changed".into());
        }
        pending.decision.shape.check(&pending.answer)?;
        let original = &pending.decision;
        if current.shape == original.shape && current.candidates == original.candidates {
            return Ok(());
        }
        let index = |i| {
            current
                .index(original.candidate(i)?)
                .ok_or_else(|| String::from("selected candidate no longer offered"))
        };
        let answer = match (&original.shape, &current.shape, &pending.answer) {
            (
                Shape::Pick { slots: old, .. },
                Shape::Pick { slots: new, .. },
                Answer::Pick { picks },
            ) => {
                let mut remapped = vec![vec![]; new.len()];
                for (slot, pick) in old.iter().zip(picks) {
                    if pick.is_empty() {
                        continue;
                    }
                    let subject = slot.subject.map(index).transpose()?;
                    let target = new
                        .iter()
                        .position(|s| s.role == slot.role && s.subject == subject)
                        .ok_or("selected slot no longer available")?;
                    remapped[target] = pick.iter().copied().map(index).collect::<Result<_, _>>()?;
                }
                Answer::Pick { picks: remapped }
            }
            (Shape::Order { .. }, Shape::Order { .. }, Answer::Order { order }) => Answer::Order {
                order: order.iter().copied().map(index).collect::<Result<_, _>>()?,
            },
            (
                Shape::Pay { .. },
                Shape::Pay { .. },
                Answer::Pay {
                    pay: PayAnswer::Activate { source },
                },
            ) => Answer::Pay {
                pay: PayAnswer::Activate {
                    source: index(*source)?,
                },
            },
            (
                Shape::Distribute { buckets: old, .. },
                Shape::Distribute { buckets: new, .. },
                Answer::Distribute { amounts },
            ) => {
                let mut remapped = vec![Quantity::zero(); new.len()];
                for (b, n) in old.iter().zip(amounts) {
                    let i = index(b.candidate)?;
                    let target = new
                        .iter()
                        .position(|bucket| bucket.candidate == i)
                        .ok_or("damage recipient changed")?;
                    remapped[target] = n.clone();
                }
                Answer::Distribute { amounts: remapped }
            }
            _ => pending.answer.clone(),
        };
        current.shape.check(&answer)
    }
}
