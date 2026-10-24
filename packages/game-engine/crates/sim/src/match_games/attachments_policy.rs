//! P6.7 completion choices use permitted rows and the public catalog only.
use super::*;
use mtg_contracts::{ActivationCost, KeywordAbility};
use mtg_core::{PermittedView, VisibleObject};
impl Strategy<'_> {
    pub(super) fn attachments(&self) -> bool {
        self.removal()
            || (self.catalog.schema_version == 17 && self.catalog.definitions.len() == 118)
    }
    pub(super) fn targetable(&self, v: &PermittedView, o: &VisibleObject) -> bool {
        !o.keywords.contains(&KeywordAbility::Shroud)
            && (o.controller == v.viewer || !o.keywords.contains(&KeywordAbility::Hexproof))
    }
    fn best_attachment_creature<'a>(
        &self,
        v: &'a PermittedView,
        own: bool,
        commander: bool,
    ) -> Option<&'a VisibleObject> {
        v.objects
            .iter()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && (o.controller == v.viewer) == own
                    && o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.kind == CardKind::Creature)
                    && self.targetable(v, o)
            })
            .max_by_key(|o| {
                (
                    commander && v.commanders.iter().any(|c| c.oracle_id == o.oracle_id),
                    o.characteristics
                        .as_ref()
                        .and_then(|c| c.power.clone())
                        .unwrap_or_default(),
                    o.handle.0.clone(),
                )
            })
    }
    pub(super) fn attachment_priority(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        for name in [
            "Swiftfoot Boots",
            "Lightning Greaves",
            "Loxodon Warhammer",
            "Whispersilk Cloak",
            "Vulshok Morningstar",
            "Bonesplitter",
            "Short Sword",
        ] {
            let Some(o) = v.objects.iter().find(|o| {
                o.zone == Zone::Battlefield
                    && o.controller == v.viewer
                    && definition(self.catalog, &o.oracle_id).is_ok_and(|d| d.name == name)
            }) else {
                continue;
            };
            let Some(target) = self.best_attachment_creature(
                v,
                true,
                matches!(name, "Swiftfoot Boots" | "Lightning Greaves"),
            ) else {
                continue;
            };
            if o.attached_to.as_ref() == Some(&target.handle) {
                continue;
            }
            if let Some(current) = o
                .attached_to
                .as_ref()
                .and_then(|h| v.objects.iter().find(|c| &c.handle == h))
            {
                let power = |c: &VisibleObject| {
                    c.characteristics
                        .as_ref()
                        .and_then(|c| c.power.clone())
                        .unwrap_or_default()
                };
                let target_commander = v.commanders.iter().any(|c| c.oracle_id == target.oracle_id);
                if current.controller == v.viewer
                    && power(current) >= power(target)
                    && !(matches!(name, "Swiftfoot Boots" | "Lightning Greaves")
                        && target_commander)
                {
                    continue;
                }
            }

            // Preserve an existing commander attachment even if another source
            // now grants that commander shroud, or there is no legal upgrade.
            if o.attached_to.as_ref().is_some_and(|h| {
                v.objects.iter().any(|c| {
                    &c.handle == h && v.commanders.iter().any(|cmd| cmd.oracle_id == c.oracle_id)
                })
            }) && matches!(name, "Swiftfoot Boots" | "Lightning Greaves")
            {
                continue;
            }
            let d = definition(self.catalog, &o.oracle_id)?;
            let ActivationCost::Mana { cost } = &d.activated[0].cost else {
                return Err("equip cost".into());
            };
            if let Some(plan) = mana_policy::plan(self.catalog, v, cost, None) {
                self.mana_plan = plan;
                self.beneficial = true;
                return Ok(Some(PlayerResponse::ActivateAbility {
                    object: o.handle.clone(),
                    ability: 0,
                }));
            }
        }
        Ok(None)
    }
    pub(super) fn attachment_target(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
    ) -> Result<Option<ObjectHandle>, String> {
        let Some(top) = v.stack.last() else {
            return Ok(None);
        };
        let (d, equip) = if let Some(a) = &top.ability {
            (definition(self.catalog, &a.oracle_id)?, true)
        } else if let Some(h) = &top.spell {
            let o = v
                .objects
                .iter()
                .find(|o| &o.handle == h)
                .ok_or("missing Aura spell row")?;
            (definition(self.catalog, &o.oracle_id)?, false)
        } else {
            return Ok(None);
        };
        if !equip && d.characteristics.subtypes != ["Aura"] {
            return Ok(None);
        }
        let own = d.name != "Pacifism";
        let prefer_commander =
            equip && matches!(d.name.as_str(), "Swiftfoot Boots" | "Lightning Greaves");
        Ok(v.objects
            .iter()
            .filter(|o| candidates.contains(&o.handle) && (o.controller == v.viewer) == own)
            .max_by_key(|o| {
                (
                    prefer_commander && v.commanders.iter().any(|c| c.oracle_id == o.oracle_id),
                    o.characteristics
                        .as_ref()
                        .and_then(|c| c.power.clone())
                        .unwrap_or_default(),
                    o.handle.0.clone(),
                )
            })
            .map(|o| o.handle.clone()))
    }
}
/// Trusted evidence adapter, independent of the completion policy.
pub(super) fn interactions(
    catalog: &Catalog,
    e: &Evidence,
    physical: &[mtg_core::CardSetup],
    g: &Game,
) -> Result<serde_json::Value, String> {
    let lookup = |card: mtg_core::CardId| -> Result<&mtg_contracts::Definition, String> {
        let row = physical
            .iter()
            .find(|c| c.id == card)
            .ok_or("attachment physical card")?;
        definition(catalog, &row.oracle_id)
    };
    let mut equips = 0u64;
    let mut auras = 0u64;
    let mut sbas = 0u64;
    let mut returns = 0u64;
    for event in g.events() {
        match event {
            mtg_core::Event::Committed {
                actual: mtg_core::EventOutcome::AttachmentChanged { object, target, .. },
                ..
            } => {
                if target.is_none() {
                    sbas += 1;
                } else if lookup(object.card)?.characteristics.subtypes == ["Equipment"] {
                    equips += 1;
                } else {
                    auras += 1;
                }
            }
            mtg_core::Event::Committed {
                actual: mtg_core::EventOutcome::Moved { movement },
                ..
            } => {
                // Creature tokens can move during Murder/Day of Judgment,
                // but have no entry in the physical-card construction table.
                // They cannot be an Aura or Rancor in this finite pool.
                let Some(card) = physical.iter().find(|c| c.id == movement.previous.card) else {
                    continue;
                };
                let d = definition(catalog, &card.oracle_id)?;
                if d.characteristics.subtypes == ["Aura"]
                    && movement.from == Zone::Battlefield
                    && matches!(movement.destination, Zone::Graveyard { .. })
                {
                    sbas += 1;
                }
                if d.name == "Rancor"
                    && matches!(movement.from, Zone::Graveyard { .. })
                    && matches!(movement.destination, Zone::Hand { .. })
                {
                    returns += 1;
                }
            }
            _ => {}
        }
    }
    // Replay each accepted target decision and count each otherwise-domain-
    // eligible permanent excluded by one of the two targeting restrictions.
    // Shroud takes precedence so each exclusion is counted once per scan.
    let mut replay = Game::with_trace(
        Prepared::admit_pod(catalog.clone(), e.config.clone()).map_err(|e| format!("{e:?}"))?,
        TraceMode::Off,
    );
    if !e.journal.concessions.is_empty() {
        return Err("completion diagnostics require natural games".into());
    }
    let mut hexproof = 0u64;
    let mut shroud = 0u64;
    for input in &e.journal.decisions {
        let Advance::AwaitingDecision(r) = replay.advance(Budget::unlimited()) else {
            return Err("attachment diagnostic decision".into());
        };
        if let mtg_core::Request::Targets { occurrences, .. }
        | mtg_core::Request::EntityTargets { occurrences, .. } = &r
        {
            let domain = occurrences
                .first()
                .ok_or("empty target occurrences")?
                .domain;
            if matches!(
                domain,
                mtg_contracts::TargetDomain::BattlefieldCreature
                    | mtg_contracts::TargetDomain::ControlledBattlefieldCreature
                    | mtg_contracts::TargetDomain::AnyDamageTarget
            ) {
                let QueryResult::Complete(QueryValue::Observation(o)) =
                    replay.observe(r.player(), Budget::unlimited())
                else {
                    return Err("attachment diagnostic observation".into());
                };
                for object in &o.view.objects {
                    if object.zone != Zone::Battlefield
                        || !object
                            .characteristics
                            .as_ref()
                            .is_some_and(|c| c.kind == CardKind::Creature)
                        || (domain == mtg_contracts::TargetDomain::ControlledBattlefieldCreature
                            && object.controller != r.player())
                    {
                        continue;
                    }
                    if object.keywords.contains(&KeywordAbility::Shroud) {
                        shroud += 1;
                    } else if object.controller != r.player()
                        && object.keywords.contains(&KeywordAbility::Hexproof)
                    {
                        hexproof += 1;
                    }
                }
            }
        }
        replay.respond(input.player, input.decision_id, input.response.clone())?;
    }
    match replay.advance(Budget::unlimited()) {
        Advance::Finished { .. } => {}
        x => return Err(format!("attachment diagnostic final: {x:?}")),
    }
    if replay.state_digest()? != e.final_state_sha256 {
        return Err("attachment diagnostic replay digest".into());
    }
    Ok(
        serde_json::json!({"equips":equips,"aura_attachments":auras,"attachment_sbas":sbas,"targets_excluded_by_hexproof":hexproof,"targets_excluded_by_shroud":shroud,"rancor_returns":returns,"exclusion_unit":"objects per target-decision candidate scan; shroud takes precedence"}),
    )
}
