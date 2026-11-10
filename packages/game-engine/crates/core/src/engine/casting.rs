use super::*;

impl Game {
    pub(super) fn validate_mana_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), String> {
        if self.is_permanent_mana(object) {
            return self.validate_permanent_mana_activation(player, object);
        }
        if self.reserved_mana_source(object) {
            return Err("mana source is already announcing".into());
        }
        let o = self.current(object)?;
        if o.controller != player || o.tapped || self.mana_abilities(object)?.len() != 1 {
            return Err("invalid mana activation".into());
        }
        Ok(())
    }

    // CR 601.2g / 605.3a-b: intrinsic mana abilities resolve immediately,
    // including in the casting mana window. No player receives priority here.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn activate_intrinsic_mana(
        &mut self,
        input: &AcceptedInput,
        object: ObjectId,
    ) -> Result<ManaActivationUndo, String> {
        self.validate_mana_activation(input.player, object)?;
        let color = *self
            .mana_abilities(object)?
            .first()
            .ok_or("missing intrinsic mana ability")?;
        let receipt = ManaActivationUndo {
            permanent: None,
            object,
            player: input.player,
            color,
            amount: Quantity::from(1),
            mana_was_absent: !self.state.rules.players[input.player.0 as usize]
                .mana
                .contains_key(&color),
        };
        self.commit_batch(
            BatchCause::ManaActivation {
                decision_id: input.decision_id,
                player: input.player,
                source: object,
            },
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Tap { object },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::AddMana {
                            player: input.player,
                            color,
                            amount: receipt.amount.clone(),
                            source: object,
                        },
                    },
                ],
            },
        )?;
        Ok(receipt)
    }

    // CR 733.1-2: undo only this attempted cast. Legal mana abilities may be
    // kept; this bounded profile offers keep-all or undo-all. External player
    // departures and mana floated before the attempt cannot be reversed.
    pub(super) fn reverse_cast(
        &mut self,
        cast: Casting,
        reason: &str,
        undo_mana: bool,
    ) -> Result<(), String> {
        let stack = self.top()?.clone();
        if stack.id != cast.stack_id || !self.is_live(stack.controller) {
            return Err("casting reversal source/player invariant".into());
        }
        if undo_mana && !cast.mana_activations.is_empty() {
            let mut children = vec![];
            for receipt in cast.mana_activations.iter().rev() {
                if let Some(permanent) = &receipt.permanent {
                    children.push(EventGroup::Occurrence {
                        proposed: ProposedEvent::ReversePermanentMana {
                            receipt: permanent.clone(),
                        },
                    });
                    continue;
                }
                let source = match self.current(receipt.object) {
                    Ok(o) if o.zone == Zone::Battlefield && o.tapped => Some(o.id),
                    Ok(_) => return Err("casting reversal source changed unexpectedly".into()),
                    Err(_) if !self.is_live(self.state.rules.cards[&receipt.object.card].owner) => {
                        None
                    }
                    Err(_) => return Err("casting reversal lost surviving mana source".into()),
                };
                children.push(EventGroup::Occurrence {
                    proposed: ProposedEvent::ReverseMana {
                        player: receipt.player,
                        color: receipt.color,
                        amount: receipt.amount.clone(),
                        source,
                        remove_zero: receipt.mana_was_absent,
                    },
                });
            }
            self.commit_batch(
                BatchCause::CastReversal {
                    stack_id: cast.stack_id,
                    player: stack.controller,
                },
                EventGroup::Ordered { children },
            )?;
        }
        self.state.rules.stack.pop();
        if cast.origin.is_spell() {
            let o = self
                .state
                .rules
                .objects
                .get_mut(&cast.original_object.card)
                .ok_or("missing reversal object")?;
            o.id = cast.original_object;
            o.zone = stack.cast_from;
            o.controller = stack.controller;
            o.default_controller = stack.controller;
            o.tapped = false;
        }
        self.state.rules.pending = None;
        self.state.rules.priority = stack.controller;
        // CR 117.4: keeping a legal mana activation interrupts the old pass
        // round. Undoing all activations restores the pre-attempt live passes.
        let live = self.live_players();
        self.state.rules.passes = if undo_mana || cast.mana_activations.is_empty() {
            cast.prior_passes
                .into_iter()
                .filter(|p| live.contains(p))
                .collect()
        } else {
            vec![]
        };
        self.state.rules.frame = Frame::Priority;
        self.record_event(Event::CastReversed {
            stack_id: cast.stack_id,
            reason: reason.into(),
        })?;
        Ok(())
    }
}
