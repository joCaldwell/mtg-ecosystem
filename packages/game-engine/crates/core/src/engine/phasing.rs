//! Private pre-event phasing selection and return timing, CR 502.1, 702.26a,
//! 702.26d, 702.26g–i, 702.26n. Phasing uses exact battlefield incarnations.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Rule-owned projection of phased-out incarnations. Each indirect member is
/// tied to its parent, even when the instruction also selected it directly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhasingSchedule {
    members: BTreeMap<ObjectId, PhaseReturn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PhaseReturn {
    Direct { controller_at_phase_out: PlayerId },
    Indirect { attached_to: ObjectId },
}

#[derive(Clone, Copy)]
pub enum PhaseSelection {
    Controlled(PlayerId),
    Target(ObjectId),
}

impl PhasingSchedule {
    pub fn return_condition(&self, object: ObjectId) -> Option<PhaseReturn> {
        self.members.get(&object).copied()
    }

    /// Construct the simultaneous status transition from one unmodified game
    /// state. Objects already phased out cannot be selected a second time.
    /// The caller owns the eventual event and the installation of this projection.
    pub fn phase_out_controlled(
        &mut self,
        game: &RulesTxn,
        controller: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        self.phase_out(game, PhaseSelection::Controlled(controller))
    }

    pub(super) fn phase_out(
        &mut self,
        game: &RulesTxn,
        selection: PhaseSelection,
    ) -> Result<Vec<ObjectId>, String> {
        match selection {
            PhaseSelection::Controlled(player) if !game.is_live(player) => {
                return Err("phasing requires a live invocation controller".into());
            }
            PhaseSelection::Target(id)
                if !game.current(id)?.present_on_battlefield()
                    || self.members.contains_key(&id) =>
            {
                return Err("phasing target is absent".into());
            }
            _ => {}
        }
        let mut direct = BTreeSet::new();
        let mut attachments = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
        for o in game.state.rules.objects.values() {
            if o.zone != Zone::Battlefield || self.members.contains_key(&o.id) {
                continue;
            }
            let selected = match selection {
                PhaseSelection::Controlled(player) => o.controller == player,
                PhaseSelection::Target(id) => o.id == id,
            };
            if selected {
                direct.insert(o.id);
            }
            if let Some(parent) = o.attached_to {
                attachments.entry(parent).or_default().push(o.id);
            }
        }
        let mut selected = direct.clone();
        let mut frontier: Vec<_> = direct.into_iter().collect();
        while let Some(id) = frontier.pop() {
            if let Some(children) = attachments.get(&id) {
                for &child in children {
                    if selected.insert(child) {
                        frontier.push(child);
                    }
                }
            }
        }
        let mut changes = BTreeMap::new();
        for &id in &selected {
            let o = game.current(id)?;
            let status = match o.attached_to {
                Some(parent) if selected.contains(&parent) => PhaseReturn::Indirect {
                    attached_to: parent,
                },
                _ => PhaseReturn::Direct {
                    controller_at_phase_out: o.controller,
                },
            };
            changes.insert(id, status);
        }
        for &id in &selected {
            let mut path = BTreeSet::new();
            let mut current = id;
            while let Some(PhaseReturn::Indirect { attached_to }) = changes.get(&current) {
                if !path.insert(current) {
                    return Err("cyclic phasing attachment selection".into());
                }
                current = *attached_to;
            }
        }
        self.members.extend(changes);
        Ok(selected.into_iter().collect())
    }

    /// CR 702.26n: the cyclic interval of seats passed on the transition
    /// to this untap includes any departed seat whose turn would have begun.
    /// Extra turns and skipped untaps are outside the existing scheduler.
    pub fn phase_in_at_untap(
        &mut self,
        game: &RulesTxn,
        previous_turn_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        if previous_turn_owner.0 >= 4 {
            return Err("invalid previous phasing seat".into());
        }
        let active = game
            .next_live(previous_turn_owner)
            .ok_or("no live untap owner")?;
        let mut due = BTreeSet::new();
        for distance in 1..=4 {
            let seat = PlayerId((previous_turn_owner.0 + distance) % 4);
            due.insert(seat);
            if seat == active {
                break;
            }
        }
        self.members
            .retain(|id, _| game.zone(*id) == Some(Zone::Battlefield));
        let mut attachments = BTreeMap::<ObjectId, Vec<ObjectId>>::new();
        let mut ready = BTreeSet::new();
        for (&id, &status) in &self.members {
            match status {
                PhaseReturn::Direct {
                    controller_at_phase_out,
                } if due.contains(&controller_at_phase_out) => {
                    ready.insert(id);
                }
                PhaseReturn::Indirect { attached_to } => {
                    attachments.entry(attached_to).or_default().push(id)
                }
                _ => {}
            }
        }
        let mut frontier: Vec<_> = ready.iter().copied().collect();
        while let Some(parent) = frontier.pop() {
            if let Some(children) = attachments.get(&parent) {
                for &id in children {
                    if ready.insert(id) {
                        frontier.push(id);
                    }
                }
            }
        }
        for id in &ready {
            self.members.remove(id);
        }
        Ok(ready.into_iter().collect())
    }
}

impl RulesTxn {
    pub(super) fn installed_phasing(&self) -> PhasingSchedule {
        PhasingSchedule {
            members: self
                .state
                .rules
                .objects
                .values()
                .filter_map(|o| o.phased_out.map(|status| (o.id, status)))
                .collect(),
        }
    }

    pub(super) fn validate_phasing_state(&self) -> Result<(), String> {
        for object in self.state.rules.objects.values() {
            let Some(status) = object.phased_out else {
                continue;
            };
            if object.zone != Zone::Battlefield
                || self.state.rules.blocking_creatures.contains(&object.id)
                || self
                    .state
                    .rules
                    .combat
                    .iter()
                    .any(|a| a.attacker == object.id || a.blockers.contains(&object.id))
            {
                return Err("invalid phased-out battlefield or combat membership".into());
            }
            match status {
                PhaseReturn::Direct {
                    controller_at_phase_out,
                } if controller_at_phase_out.0 >= 4 => {
                    return Err("invalid phasing return seat".into());
                }
                PhaseReturn::Indirect { attached_to } => {
                    if object.attached_to != Some(attached_to) {
                        return Err("invalid indirect phasing attachment".into());
                    }
                    let mut seen = BTreeSet::from([object.id]);
                    let mut parent = attached_to;
                    while let Ok(host) = self.current(parent) {
                        if !seen.insert(parent) {
                            return Err("cyclic indirect phasing return".into());
                        }
                        match host.phased_out {
                            Some(PhaseReturn::Indirect { attached_to }) => parent = attached_to,
                            Some(PhaseReturn::Direct { .. }) => break,
                            None => {
                                return Err("indirectly phased attachment has present host".into());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn phase_out_permanents(
        &mut self,
        selection: PhaseSelection,
    ) -> Result<EventOutcome, String> {
        let controller = match selection {
            PhaseSelection::Controlled(controller) => controller,
            PhaseSelection::Target(id) => self.current(id)?.controller,
        };
        let mut schedule = self.installed_phasing();
        let ids = schedule.phase_out(self, selection)?;
        for &id in &ids {
            self.retain_source_lki(id)?;
        }
        let mut members = Vec::with_capacity(ids.len());
        for id in ids {
            let status = schedule
                .return_condition(id)
                .ok_or("missing phased status")?;
            self.rules_mut()
                .objects
                .get_mut(&id.card)
                .unwrap()
                .phased_out = Some(status);
            self.remove_from_combat(id);
            members.push((id, status));
        }
        Ok(EventOutcome::PermanentsPhasedOut {
            controller,
            members,
        })
    }

    pub(crate) fn phase_in_for_next_untap(
        &mut self,
        previous_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        let mut schedule = self.installed_phasing();
        if schedule.members.is_empty() {
            return Ok(vec![]);
        }
        let ids = schedule.phase_in_at_untap(self, previous_owner)?;
        for id in &ids {
            let object = self.current(*id)?;
            let lost_host = matches!(object.phased_out, Some(PhaseReturn::Direct { .. }))
                && object
                    .attached_to
                    .is_some_and(|host| self.zone(host) != Some(Zone::Battlefield));
            let returning = self.rules_mut().objects.get_mut(&id.card).unwrap();
            returning.phased_out = None;
            if lost_host {
                returning.attached_to = None;
                returning.attachment_timestamp = None;
            }
        }
        self.reconcile_controllers()?;
        Ok(ids)
    }
}
