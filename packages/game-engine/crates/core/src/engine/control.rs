//! Layer 2 for complete permanent-control Auras. Pinned CR 109.5, 110.2,
//! 302.6, 506.4, 611.3, 613.1b/7a/7e/8a-c and 800.4a-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Scratch-only layer-2 instruction. The complete source recognizer excludes
/// text/type/ability-changing effects; only the meaning of "you" can depend on
/// another layer-2 effect. No final cached controller is an input.
#[derive(Clone, Copy)]
pub struct AuraControlEffect {
    pub source: usize,
    pub target: usize,
    pub timestamp: u64,
}

/// CR 109.5/611.3a/613.8a: the structural link is only a potential
/// dependency. Applying the other effect must *change* what this effect does.
/// This is a derivation of the pinned rules, not an official worked-example
/// ruling: assigning the same source controller leaves the recipient player
/// unchanged. Compare attempted operations, NOT whether this effect's target
/// already has that controller. A departed-player grant cannot apply (800.4b).
/// Test against the current scratch view, never final winners.
fn aura_control_depends(
    effect: AuraControlEffect,
    other: AuraControlEffect,
    controllers: &[PlayerId],
    live: &[PlayerId],
) -> bool {
    other.target == effect.source
        && live.contains(&controllers[other.source])
        && controllers[other.source] != controllers[effect.source]
}

/// The admitted producer always attaches a fresh Aura to an existing permanent;
/// it cannot create a source-attachment cycle. Reject structural cycles even if
/// equal controllers would hide their current semantic edges. General synthetic
/// CR 613.8b loop interpretation remains outside this runtime's source boundary.
fn validate_aura_control_graph(
    effects: &[AuraControlEffect],
    source_effects: &[Option<usize>],
) -> Result<(), String> {
    let mut prerequisites = vec![0; effects.len()];
    for effect in effects {
        if let Some(dependent) = source_effects[effect.target] {
            prerequisites[dependent] += 1;
        }
    }
    let mut ready: Vec<_> = prerequisites
        .iter()
        .enumerate()
        .filter_map(|(i, &count)| (count == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = ready.pop() {
        visited += 1;
        if let Some(dependent) = source_effects[effects[i].target] {
            prerequisites[dependent] -= 1;
            if prerequisites[dependent] == 0 {
                ready.push(dependent);
            }
        }
    }
    if visited != effects.len() {
        return Err("unsupported cyclic Aura control sources".into());
    }
    Ok(())
}

/// CR 613.7/613.8a,c: select the earliest remaining effect with no current
/// semantic prerequisites, apply ONE, then reevaluate. Each effect targets one
/// object, which generates at most one control effect: at most A potential
/// edges including fan-in, so each pass is O(A), not a pairwise scan.
fn next_aura_control_effect(
    effects: &[AuraControlEffect],
    applied: &[bool],
    source_effects: &[Option<usize>],
    controllers: &[PlayerId],
    live: &[PlayerId],
) -> Option<usize> {
    let mut blocked = vec![false; effects.len()];
    for (other, grant) in effects.iter().enumerate() {
        if !applied[other]
            && let Some(effect) = source_effects[grant.target]
            && !applied[effect]
            && aura_control_depends(effects[effect], *grant, controllers, live)
        {
            blocked[effect] = true;
        }
    }
    (0..effects.len())
        .filter(|&i| !applied[i] && !blocked[i])
        .min_by_key(|&i| effects[i].timestamp)
}

/// Dense battlefield indices bound scratch work to O(B + A²). No fixed-point,
/// cached final controller, recursive winner-following or persistent graph.
pub fn apply_aura_control_effects(
    effects: &[AuraControlEffect],
    controllers: &mut [PlayerId],
    live: &[PlayerId],
) -> Result<(), String> {
    let mut source_effects = vec![None; controllers.len()];
    for (i, effect) in effects.iter().enumerate() {
        source_effects[effect.source] = Some(i);
    }
    validate_aura_control_graph(effects, &source_effects)?;
    let mut applied = vec![false; effects.len()];
    for _ in effects {
        let next = next_aura_control_effect(effects, &applied, &source_effects, controllers, live)
            .ok_or("unsupported cyclic Aura control dependencies")?;
        let effect = effects[next];
        applied[next] = true;
        // A dead-default source can first be rescued by another Aura. A source
        // still dead when its effect applies grants nothing before 800.4a/c exile.
        if live.contains(&controllers[effect.source]) {
            controllers[effect.target] = controllers[effect.source];
        }
    }
    Ok(())
}

impl RulesTxn {
    fn supported_control_grant(&self, effect: &ControlEffect) -> bool {
        self.is_live(effect.controller)
            && match effect.source {
                ControlSource::Persistent => true,
                ControlSource::Static { object } => self.control_battlefield(object),
            }
    }

    /// CR 800.4a: all frozen losing owners' objects are absent from the virtual
    /// battlefield even during the existing sequential physical removal wave.
    fn control_battlefield(&self, object: ObjectId) -> bool {
        self.current(object).is_ok_and(|o| {
            o.present_on_battlefield() && self.is_live(self.state.rules.cards[&object.card].owner)
        })
    }

    /// Derive the complete post-mutation view before changing any cached value.
    /// Synthetic grants retain their existing authority; they have no timestamp
    /// in the Aura clock, so composing the two orders is explicitly unsupported.
    pub(crate) fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        let mut controllers = BTreeMap::new();
        let mut granted = BTreeSet::new();
        for object in self.state.rules.objects.values() {
            if !self.control_battlefield(object.id) {
                continue;
            }
            let grant = self
                .state
                .rules
                .control_effects
                .iter()
                .filter(|effect| effect.object == object.id && self.supported_control_grant(effect))
                .max_by_key(|effect| effect.id);
            if grant.is_some() {
                granted.insert(object.id);
            }
            controllers.insert(
                object.id,
                grant.map_or(object.default_controller, |effect| effect.controller),
            );
        }

        let indices: BTreeMap<_, _> = controllers
            .keys()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
        let mut scratch: Vec<_> = controllers.values().copied().collect();
        let mut effects = vec![];
        let mut timestamps = BTreeSet::new();
        for source in self.state.rules.objects.values() {
            if !self.control_battlefield(source.id) {
                continue;
            }
            let definition = self.copiable_definition(source.id)?;
            if !definition
                .static_abilities
                .iter()
                .any(|ability| ability.effect == StaticEffect::ControlEnchantedPermanent)
            {
                continue;
            }
            if !mtg_contracts::aura_control_source(definition) {
                return Err("unsupported dependent Aura control source".into());
            }
            let Some(target) = source.attached_to else {
                continue;
            };
            // A stale pre-SBA attachment is reachable, but supplies no grant.
            // Never follow a physical card to its successor incarnation.
            if !self.control_battlefield(target)
                || !self.attachment_target_type_valid(source.id, target)?
            {
                continue;
            }
            let timestamp = source
                .attachment_timestamp
                .filter(|timestamp| {
                    *timestamp > 0 && *timestamp < self.state.rules.next_attachment_timestamp
                })
                .ok_or("missing or invalid Aura control timestamp")?;
            if !timestamps.insert(timestamp) {
                return Err("unsupported simultaneous Aura control timestamps".into());
            }
            if granted.contains(&target) {
                return Err("unsupported composed Aura and synthetic control grants".into());
            }
            effects.push(AuraControlEffect {
                source: indices[&source.id],
                target: indices[&target],
                timestamp,
            });
        }
        apply_aura_control_effects(&effects, &mut scratch, &self.live_players())?;
        for (controller, final_controller) in controllers.values_mut().zip(scratch) {
            *controller = final_controller;
        }
        // Synthetic grants have no timestamp in the Aura clock. In particular,
        // a static grant whose exact source needs control-related exile would
        // require removing that grant and reevaluating. Preserve the explicit
        // boundary rather than publish an ID-ordered or stale interpretation.
        if self.state.rules.control_effects.iter().any(|effect| {
            self.supported_control_grant(effect)
                && controllers.contains_key(&effect.object)
                && matches!(effect.source, ControlSource::Static { object }
                    if controllers.get(&object).is_some_and(|controller| !self.is_live(*controller)))
        }) {
            return Err("unsupported dependent synthetic control-source exile".into());
        }
        Ok(controllers)
    }

    /// Only layer-2 consequences: no stabilization, trigger placement, priority
    /// change or continuation replacement. The caller owns the atomic boundary.
    pub(crate) fn reconcile_controllers(&mut self) -> Result<(), String> {
        let controllers = self.derived_controllers()?;
        self.grant_hideaway_looks(&controllers);
        let mut changed = vec![];
        let mut exiled = vec![];
        for (object, controller) in controllers {
            let previous = self.current(object)?.controller;
            if !self.is_live(controller) {
                exiled.push(object);
            } else if previous != controller {
                changed.push((object, previous, controller));
            }
        }
        if changed.is_empty() && exiled.is_empty() {
            return Ok(());
        }
        let authority = self.control_before()?;
        // Publish the complete layer-2 view before deriving any layer-6 source
        // facts. Its meaning cannot depend on iteration order among objects.
        for (object, _, controller) in &changed {
            let cached = self.rules_mut().objects.get_mut(&object.card).unwrap();
            cached.controller = *controller;
            cached.attack_ready = false;
            self.remove_from_combat(*object);
        }
        for (object, previous, controller) in changed {
            let fact = self.object_before(object)?;
            self.retain_damage_control_facts(&fact);
            self.record_event(Event::ControlChanged {
                object,
                previous,
                controller,
            })?;
        }
        let prior = exiled
            .iter()
            .map(|object| self.object_before(*object))
            .collect::<Result<Vec<_>, _>>()?;
        for object in exiled {
            // A dedicated movement batch avoids commit -> apply_group ->
            // reconciliation recursion, while retaining consumed authority,
            // ordinary zone history, exact incarnation and bounded LKI.
            self.exile_without_controller(object, authority.clone())?;
        }
        for fact in prior {
            self.retain_source_facts(&fact);
        }
        Ok(())
    }

    pub(crate) fn validate_control_state(&self) -> Result<(), String> {
        for (object, controller) in self.derived_controllers()? {
            if !self.is_live(controller) || self.current(object)?.controller != controller {
                return Err("snapshot cached controller invariant".into());
            }
        }
        Ok(())
    }
}
