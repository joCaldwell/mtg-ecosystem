//! Layer-4 type replacement and layer-6 ability removal (CR 205.1a, 305.6–7,
//! 613.1d, 613.1f, 613.6–613.8).
//!
//! Layer 4 is resolved per object: resolved animations and self statics have
//! fixed recipients, while an Aura's noun and a global filter read card types
//! another layer-4 effect can change, so such an effect waits until every
//! effect that would change whether it applies has applied (CR 613.8a–c). An
//! effect exists while its source keeps the abilities its rules text
//! generates: a basic land type set on a land source removes them (CR 305.7),
//! except for the effect doing so (an effect never depends on itself).
//!
//! Layer 6 removes abilities in timestamp order with grants (CR 613.7), but an
//! effect whose source loses its abilities depends on the removal (CR 613.8a),
//! so a removal applies before the grants and removals its recipients' own
//! abilities generate. Effects that started in an earlier layer continue
//! (CR 613.6). Printed abilities are always earlier than any removal.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Animation, Change, Layer, Scope};
use std::borrow::Cow;

/// One battlefield static ability of one incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StaticKey {
    pub source: ObjectId,
    pub index: u32,
}

/// The layer-4 result of one object.
pub(super) struct TypeLayer<'a> {
    pub facts: Cow<'a, Characteristics>,
    /// CR 305.7: the abilities its rules text generates are gone.
    pub rules_text_removed: bool,
    /// The layer-4-first statics that applied, which fixes their recipients
    /// in later layers (CR 613.6).
    pub applied: Vec<StaticKey>,
}

/// A static all-ability removal that exists after layer-6 ordering.
#[derive(Clone, Copy)]
pub(super) struct Removal<'a> {
    pub key: StaticKey,
    pub source: &'a Object,
    affects: &'a Scope,
    first: Layer,
    time: u64,
    rank: u8,
}

fn first_layer(changes: &[Change]) -> Option<Layer> {
    changes.iter().map(Change::layer).min()
}

/// CR 205.1a: canonical primary type, then the others in order.
fn set_card_types(facts: &mut Characteristics, kinds: &[CardKind], printed: &Characteristics) {
    let had_creature = facts.has_type(CardKind::Creature);
    let Some(primary) = [
        CardKind::Creature,
        CardKind::Land,
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Instant,
        CardKind::Sorcery,
    ]
    .into_iter()
    .find(|k| kinds.contains(k)) else {
        return;
    };
    facts.kind = primary;
    facts.additional_types = kinds.iter().copied().filter(|k| *k != primary).collect();
    facts.additional_types.sort_unstable();
    facts.additional_types.dedup();
    // CR 205.1a: subtypes stay only with a correlated remaining card type.
    let keeps = |facts: &Characteristics, s: &str| match mtg_contracts::kir::subtype_card_type(s) {
        None => true,
        Some(CardKind::Instant | CardKind::Sorcery) => {
            facts.has_type(CardKind::Instant) || facts.has_type(CardKind::Sorcery)
        }
        Some(kind) => facts.has_type(kind),
    };
    let subtypes = std::mem::take(&mut facts.subtypes);
    facts.subtypes = subtypes.into_iter().filter(|s| keeps(facts, s)).collect();
    if facts.has_type(CardKind::Creature) {
        // CR 208.1: a creature has power and toughness; a later 7b setter of
        // the same effect supplies them for a card printed without.
        if !had_creature {
            facts.power = Some(printed.power.clone().unwrap_or_default());
            facts.toughness = Some(printed.toughness.clone().unwrap_or_default());
        }
    } else {
        facts.all_creature_types = false;
        facts.power = None;
        facts.toughness = None;
    }
}

fn apply_animation(facts: &mut Characteristics, animation: &Animation, printed: &Characteristics) {
    let had_creature = facts.has_type(CardKind::Creature);
    for kind in [CardKind::Creature, CardKind::Artifact] {
        if (kind == CardKind::Creature || animation.add_artifact) && !facts.has_type(kind) {
            facts.additional_types.push(kind);
        }
    }
    facts.additional_types.sort_unstable();
    if !had_creature && facts.power.is_none() {
        facts.power = printed.power.clone();
        facts.toughness = printed.toughness.clone();
    }
    if animation.replace_creature_subtypes {
        facts
            .subtypes
            .retain(|s| !mtg_contracts::kir::is_creature_subtype(s));
        facts.all_creature_types = false;
    }
    for subtype in &animation.creature_subtypes {
        if !facts.subtypes.contains(subtype) {
            facts.subtypes.push(subtype.clone());
        }
    }
}

/// One layer-4 change in the order the effect lists them (CR 613.6).
fn apply_type_change(
    facts: &mut Characteristics,
    change: &Change,
    printed: &Characteristics,
    rules_text_removed: &mut bool,
) {
    match change {
        Change::AddTypes { kinds } => {
            for kind in kinds {
                if !facts.has_type(*kind) {
                    facts.additional_types.push(*kind);
                    facts.additional_types.sort_unstable();
                }
            }
        }
        // CR 205.3d: only a creature can gain a creature type.
        Change::AddCreatureSubtypes { subtypes } if facts.has_type(CardKind::Creature) => {
            for subtype in subtypes {
                if !facts.subtypes.contains(subtype) {
                    facts.subtypes.push(subtype.clone());
                }
            }
        }
        Change::AllCreatureTypes => facts.all_creature_types = true,
        Change::SetCardTypes { kinds } => set_card_types(facts, kinds, printed),
        Change::RemoveCardTypes { kinds } => {
            let remaining: Vec<_> = std::iter::once(facts.kind)
                .chain(facts.additional_types.iter().copied())
                .filter(|k| !kinds.contains(k))
                .collect();
            if !remaining.is_empty() {
                set_card_types(facts, &remaining, printed);
            }
        }
        // CR 205.3d: only a creature can have a creature type.
        Change::SetCreatureSubtypes { subtypes } if facts.has_type(CardKind::Creature) => {
            facts
                .subtypes
                .retain(|s| !mtg_contracts::kir::is_creature_subtype(s));
            facts.all_creature_types = false;
            facts.subtypes.extend(subtypes.iter().cloned());
        }
        Change::SetLandSubtypes { subtypes } if facts.has_type(CardKind::Land) => {
            facts
                .subtypes
                .retain(|s| !mtg_contracts::kir::is_land_subtype(s));
            facts.subtypes.extend(subtypes.iter().cloned());
            *rules_text_removed = true;
        }
        _ => {}
    }
}

/// Each recipient with whether each of the effect's layer-4 changes takes
/// effect there.
type Application = Vec<(ObjectId, Vec<bool>)>;

fn members(application: &Application) -> Vec<ObjectId> {
    application.iter().map(|(o, _)| *o).collect()
}

/// Whether a layer-4 change does anything to `facts` (CR 205.3d: only a
/// creature or a land can have that kind of subtype).
fn change_takes_effect(change: &Change, facts: &Characteristics) -> bool {
    match change {
        Change::SetCreatureSubtypes { .. } | Change::AddCreatureSubtypes { .. } => {
            facts.has_type(CardKind::Creature)
        }
        Change::SetLandSubtypes { .. } => facts.has_type(CardKind::Land),
        Change::RemoveCardTypes { kinds } => kinds.iter().any(|k| facts.has_type(*k)),
        _ => true,
    }
}

/// One layer-4 effect: a resolved animation with its captured
/// incarnations, or a static ability.
struct Eff<'a> {
    /// (timestamp, static before resolved (as in layer 6), APNAP rank,
    /// creation).
    order: (u64, u8, u8, usize),
    kind: EffKind<'a>,
}

enum EffKind<'a> {
    Resolved {
        targets: Vec<ObjectId>,
        animation: &'a Animation,
    },
    Static {
        key: StaticKey,
        source: &'a Object,
        affects: &'a Scope,
        changes: &'a [Change],
    },
}

#[derive(Clone, Debug)]
pub(in crate::engine) struct Entry {
    facts: Characteristics,
    removed: bool,
    applied: Vec<StaticKey>,
}

/// Layer-4 state while effects apply; an absent object has its copiable
/// values.
#[derive(Clone)]
struct Layer4 {
    entries: BTreeMap<ObjectId, Entry>,
}

/// The layer-4 result of every battlefield object a layer-4 effect reached.
#[derive(Debug)]
pub(in crate::engine) struct GlobalTypes {
    entries: BTreeMap<ObjectId, Entry>,
    ambiguous: bool,
}

impl RulesTxn {
    /// Every battlefield-functioning static of every battlefield source, with
    /// its index, before any existence check.
    pub(super) fn indexed_battlefield_statics<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = (&'a Object, StaticKey, &'a StaticEffect)> + 'a {
        self.battlefield_static_sources(sources)
            .flat_map(move |source| {
                self.copiable_definition(source.id)
                    .into_iter()
                    .flat_map(|d| d.static_abilities.iter().enumerate())
                    .filter(|(_, a)| a.functioning_zone == FunctioningZone::Battlefield)
                    .map(move |(index, a)| {
                        (
                            source,
                            StaticKey {
                                source: source.id,
                                index: index as u32,
                            },
                            &a.effect,
                        )
                    })
            })
    }

    fn any_static_change(
        &self,
        sources: Option<&[CardId]>,
        pred: impl Fn(&Change) -> bool,
    ) -> bool {
        self.indexed_battlefield_statics(sources).any(|(_, _, e)| {
            matches!(e, StaticEffect::Continuous { changes, .. } if changes.iter().any(&pred))
        })
    }

    /// CR 305.7: whether any static could remove rules-text abilities.
    pub(super) fn land_type_setters(&self, sources: Option<&[CardId]>) -> bool {
        self.any_static_change(sources, |c| matches!(c, Change::SetLandSubtypes { .. }))
    }

    /// Whether any static removes every ability of its recipients.
    fn ability_removers(&self, sources: Option<&[CardId]>) -> bool {
        self.any_static_change(sources, |c| matches!(c, Change::RemoveAllAbilities { .. }))
    }

    pub(super) fn type_layer(&self, object: ObjectId) -> Result<TypeLayer<'_>, String> {
        let printed = &self.copiable_definition(object)?.characteristics;
        let present = self
            .current(object)
            .is_ok_and(|o| o.id == object && o.present_on_battlefield());
        let entry = if present {
            self.global_types()?.entries.get(&object)
        } else {
            None
        };
        Ok(match entry {
            Some(e) => TypeLayer {
                facts: Cow::Borrowed(&e.facts),
                rules_text_removed: e.removed,
                applied: e.applied.clone(),
            },
            None => TypeLayer {
                facts: Cow::Borrowed(printed),
                rules_text_removed: false,
                applied: vec![],
            },
        })
    }

    /// CR 305.7: whether `source` keeps the abilities its rules text
    /// generates after layer 4.
    pub(super) fn keeps_rules_text(&self, source: ObjectId) -> Result<bool, String> {
        Ok(!self
            .global_types()?
            .entries
            .get(&source)
            .is_some_and(|e| e.removed))
    }

    /// CR 613.7m: the relative timestamps of one controller's simultaneous
    /// objects are that player's choice. The engine asks no such choice, so a
    /// state where that order would decide a layer-4 or layer-6 result is
    /// rejected; objects of different controllers go in APNAP order.
    pub(super) fn reject_simultaneous_timestamp_order(&self) -> Result<(), String> {
        const REJECT: &str = "simultaneous timestamps of one controller would decide a continuous effect (CR 613.7m)";
        if self.global_types()?.ambiguous || self.alive_removals_ordered(None)?.1 {
            return Err(REJECT.into());
        }
        let alive = self.alive_removals(None)?;
        if alive.is_empty()
            && !self.any_static_change(None, |c| matches!(c, Change::RemoveKeywords { .. }))
        {
            return Ok(());
        }
        // CR 613.7m: one rank at one timestamp is one controller's tie.
        let rank = |key: StaticKey| self.current(key.source).map(|o| o.keyword_rank).ok();
        for object in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
        {
            let rows = self.static_changes_keyed(object.id, Layer::Ability, None);
            for (time, removing, removal) in &rows {
                let removed: Option<&[KeywordAbility]> = match removal {
                    Change::RemoveAllAbilities { .. } => None,
                    Change::RemoveKeywords { keywords } => Some(keywords),
                    _ => continue,
                };
                for (other, granting, grant) in &rows {
                    let overlaps = match (grant, removed) {
                        (Change::AddKeywords { .. } | Change::AddManaAbilities { .. }, None) => {
                            true
                        }
                        (Change::AddKeywords { keywords }, Some(removed)) => {
                            keywords.iter().any(|k| removed.contains(k))
                        }
                        _ => false,
                    };
                    if overlaps
                        && other == time
                        && granting.source != removing.source
                        && rank(*granting) == rank(*removing)
                    {
                        return Err(REJECT.into());
                    }
                }
            }
        }
        Ok(())
    }

    /// The layer-4 result of the whole battlefield, cached until the next
    /// rules write (`RulesState`).
    fn global_types(&self) -> Result<&GlobalTypes, String> {
        let cache = self.state.rules.layer_cache();
        if let Some(types) = cache.get() {
            return Ok(types);
        }
        let types = std::sync::Arc::new(self.compute_global_types()?);
        let _ = cache.set(types);
        Ok(cache.get().expect("layer cache was just set"))
    }

    /// CR 613.7m: the active player's objects first, then turn order, as of
    /// the moment the timestamp is assigned (`Object::keyword_rank`).
    pub(super) fn apnap_rank(&self, player: PlayerId) -> u8 {
        let seats = u8::try_from(self.state.rules.players.len().max(1)).unwrap_or(u8::MAX);
        let active = self.state.rules.active.map_or(0, |p| p.0) % seats;
        (player.0 % seats + seats - active) % seats
    }

    fn compute_global_types(&self) -> Result<GlobalTypes, String> {
        let mut effects: Vec<Eff<'_>> = vec![];
        for effect in &self.state.rules.effects {
            if let ResolvedEffectKind::SetCharacteristics {
                targets,
                values,
                timestamp,
            } = &effect.kind
                && let Some(animation) = &values.animation
            {
                let targets: Vec<ObjectId> = targets
                    .iter()
                    .copied()
                    .filter(|t| {
                        self.current(*t)
                            .is_ok_and(|o| o.id == *t && o.present_on_battlefield())
                    })
                    .collect();
                effects.push(Eff {
                    order: (*timestamp, 1, 0, effects.len()),
                    kind: EffKind::Resolved { targets, animation },
                });
            }
        }
        for (source, key, effect) in self.indexed_battlefield_statics(None) {
            let StaticEffect::Continuous {
                affects,
                changes,
                condition,
            } = effect
            else {
                continue;
            };
            // Admitted layer-4 conditions read nothing a layer changes.
            if first_layer(changes) != Some(Layer::Type)
                || condition
                    .as_ref()
                    .is_some_and(|c| !self.static_condition(source, c))
            {
                continue;
            }
            effects.push(Eff {
                order: (
                    source.keyword_timestamp.unwrap_or(0),
                    0,
                    source.keyword_rank,
                    effects.len(),
                ),
                kind: EffKind::Static {
                    key,
                    source,
                    affects,
                    changes,
                },
            });
        }
        let mut state = Layer4 {
            entries: BTreeMap::new(),
        };
        let mut ambiguous = false;
        effects.sort_by_key(|e| e.order);
        // CR 613.8b-c: apply the earliest effect that depends on no other
        // remaining effect outside its own dependency loop; re-evaluate
        // after each application. CR 613.8a(c) never applies: no admitted
        // layer-4 effect is a characteristic-defining ability (changeling
        // is a copiable value, CR 604.3, 613.3), and `cda-types` rejects.
        // An effect depends on another when applying that one changes its
        // existence, its recipients or what it does to them.
        while !effects.is_empty() {
            let apps = effects
                .iter()
                .map(|e| self.layer4_application(e, &state))
                .collect::<Result<Vec<_>, _>>()?;
            let n = effects.len();
            let mut depends = vec![vec![false; n]; n];
            for j in 0..n {
                if apps[j].is_empty() {
                    continue;
                }
                let mut after = state.clone();
                self.apply_layer4_effect(&effects[j], &members(&apps[j]), &mut after)?;
                for i in (0..n).filter(|i| *i != j) {
                    depends[i][j] = self.layer4_application(&effects[i], &after)? != apps[i];
                }
            }
            // Transitive closure; i is ready when everything it reaches
            // reaches it back (its loop is a sink).
            let mut reach = depends.clone();
            for k in 0..n {
                let via = reach[k].clone();
                for row in &mut reach {
                    if row[k] {
                        for (cell, through) in row.iter_mut().zip(&via) {
                            *cell |= *through;
                        }
                    }
                }
            }
            let ready: Vec<usize> = (0..n)
                .filter(|&i| (0..n).all(|j| !reach[i][j] || reach[j][i]))
                .collect();
            let pick = *ready.first().ok_or("layer-4 dependency order is empty")?;
            for &other in &ready[1..] {
                if self.order_matters(&effects, pick, other, &apps, &state)? {
                    ambiguous = true;
                }
            }
            let effect = effects.remove(pick);
            self.apply_layer4_effect(&effect, &members(&apps[pick]), &mut state)?;
        }
        Ok(GlobalTypes {
            entries: state.entries,
            ambiguous,
        })
    }

    /// CR 613.7m: two ready statics with one timestamp and one APNAP rank
    /// (one controller's simultaneous objects) unless both orders give the
    /// same whole layer-4 state, including the recipients later layers use.
    fn order_matters(
        &self,
        effects: &[Eff<'_>],
        a: usize,
        b: usize,
        apps: &[Application],
        state: &Layer4,
    ) -> Result<bool, String> {
        let (x, y) = (&effects[a], &effects[b]);
        let (EffKind::Static { key: kx, .. }, EffKind::Static { key: ky, .. }) = (&x.kind, &y.kind)
        else {
            return Ok(false);
        };
        if (x.order.0, x.order.1, x.order.2) != (y.order.0, y.order.1, y.order.2)
            || kx.source == ky.source
        {
            return Ok(false);
        }
        let run = |first: usize, second: usize| -> Result<Layer4, String> {
            let mut s = state.clone();
            self.apply_layer4_effect(&effects[first], &members(&apps[first]), &mut s)?;
            let next = self.layer4_application(&effects[second], &s)?;
            self.apply_layer4_effect(&effects[second], &members(&next), &mut s)?;
            Ok(s)
        };
        let normal = |s: Layer4| {
            s.entries
                .into_iter()
                .map(|(o, mut e)| {
                    e.applied.sort_by_key(|k| (k.source, k.index));
                    (o, e.facts, e.removed, e.applied)
                })
                .collect::<Vec<_>>()
        };
        Ok(normal(run(a, b)?) != normal(run(b, a)?))
    }

    /// What `effect` does in `state` (CR 613.8a): each recipient with,
    /// for each of its layer-4 changes in order, whether that change takes
    /// effect there. Empty when its source no longer has the ability
    /// (CR 305.7).
    fn layer4_application(&self, effect: &Eff<'_>, state: &Layer4) -> Result<Application, String> {
        let facts = |target: ObjectId| -> Result<&Characteristics, String> {
            Ok(match state.entries.get(&target) {
                Some(e) => &e.facts,
                None => &self.copiable_definition(target)?.characteristics,
            })
        };
        let (source, affects, changes) = match &effect.kind {
            EffKind::Resolved { targets, .. } => {
                return Ok(targets.iter().map(|t| (*t, vec![])).collect());
            }
            EffKind::Static {
                source,
                affects,
                changes,
                ..
            } => (*source, *affects, *changes),
        };
        if state.entries.get(&source.id).is_some_and(|e| e.removed) {
            return Ok(vec![]);
        }
        let mut recipients = vec![];
        match affects {
            Scope::Source => recipients.push(source.id),
            Scope::Attachment { .. } => {
                if let Some(target) = source
                    .attached_to
                    .and_then(|t| self.current(t).ok())
                    .filter(|t| t.present_on_battlefield())
                    && self.static_matches(
                        affects,
                        source,
                        target,
                        facts(target.id)?,
                        target.controller,
                    )
                {
                    recipients.push(target.id);
                }
            }
            Scope::Matching { .. } => {
                for target in self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.present_on_battlefield())
                {
                    if self.static_matches(
                        affects,
                        source,
                        target,
                        facts(target.id)?,
                        target.controller,
                    ) {
                        recipients.push(target.id);
                    }
                }
            }
        }
        let mut out = vec![];
        for object in recipients {
            let printed = &self.copiable_definition(object)?.characteristics;
            let mut now = facts(object)?.clone();
            let effective = changes
                .iter()
                .filter(|c| c.layer() == Layer::Type)
                .map(|change| {
                    let takes = change_takes_effect(change, &now);
                    apply_type_change(&mut now, change, printed, &mut false);
                    takes
                })
                .collect();
            out.push((object, effective));
        }
        Ok(out)
    }

    fn apply_layer4_effect(
        &self,
        effect: &Eff<'_>,
        members: &[ObjectId],
        state: &mut Layer4,
    ) -> Result<(), String> {
        for object in members {
            let printed = &self.copiable_definition(*object)?.characteristics;
            let entry = state.entries.entry(*object).or_insert_with(|| Entry {
                facts: printed.clone(),
                removed: false,
                applied: vec![],
            });
            match &effect.kind {
                EffKind::Resolved { animation, .. } => {
                    apply_animation(&mut entry.facts, animation, printed)
                }
                EffKind::Static { key, changes, .. } => {
                    for change in changes.iter().filter(|c| c.layer() == Layer::Type) {
                        apply_type_change(&mut entry.facts, change, printed, &mut entry.removed);
                    }
                    entry.applied.push(*key);
                }
            }
        }
        Ok(())
    }

    /// Whether the layer-6 removal `r` applies to `target`, read in the
    /// removal's first layer (CR 613.6).
    fn removal_applies(&self, r: &Removal<'_>, target: &Object) -> Result<bool, String> {
        if !target.present_on_battlefield() {
            return Ok(false);
        }
        Ok(match r.first {
            Layer::Type => self.type_layer(target.id)?.applied.contains(&r.key),
            Layer::Color => {
                let facts = self.structural_characteristics(target.id)?;
                self.static_matches(r.affects, r.source, target, &facts, target.controller)
            }
            _ => {
                let facts = self.color_layer(target.id)?;
                self.static_matches(r.affects, r.source, target, &facts, target.controller)
            }
        })
    }

    /// The all-ability removals that exist once layer-6 dependency is
    /// ordered (CR 613.8). A removal that started in an earlier layer
    /// continues (CR 613.6); a layer-6 one that another existing removal
    /// strips from its source never applies; a loop goes by timestamp.
    pub(super) fn alive_removals<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> Result<Vec<Removal<'a>>, String> {
        Ok(self.alive_removals_ordered(sources)?.0)
    }

    /// [`Self::alive_removals`], and whether a loop's timestamp order fell
    /// to one controller's simultaneous removals (CR 613.7m).
    fn alive_removals_ordered<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> Result<(Vec<Removal<'a>>, bool), String> {
        let mut ambiguous = false;
        if !self.ability_removers(sources) {
            return Ok((vec![], false));
        }
        let setters = self.land_type_setters(sources);
        let mut alive = vec![];
        let mut pending = vec![];
        for (source, key, effect) in self.indexed_battlefield_statics(sources) {
            let StaticEffect::Continuous {
                affects, changes, ..
            } = effect
            else {
                continue;
            };
            if !changes
                .iter()
                .any(|c| matches!(c, Change::RemoveAllAbilities { .. }))
            {
                continue;
            }
            let first = first_layer(changes).ok_or("empty continuous static")?;
            let removal = Removal {
                key,
                source,
                affects,
                first,
                time: source.keyword_timestamp.unwrap_or(0),
                rank: source.keyword_rank,
            };
            if first == Layer::Type {
                // Its existence and recipients are fixed in layer 4.
                alive.push(removal);
            } else if !setters || self.keeps_rules_text(source.id)? {
                if first < Layer::Ability {
                    alive.push(removal);
                } else {
                    pending.push(removal);
                }
            }
        }
        pending.sort_by_key(|r| (r.time, r.rank));
        loop {
            let mut kept = vec![];
            for p in pending {
                let mut killed = false;
                for a in &alive {
                    if a.key != p.key && self.removal_applies(a, p.source)? {
                        killed = true;
                        break;
                    }
                }
                if !killed {
                    kept.push(p);
                }
            }
            pending = kept;
            if pending.is_empty() {
                return Ok((alive, ambiguous));
            }
            let mut pick = None;
            'candidates: for i in 0..pending.len() {
                for (j, q) in pending.iter().enumerate() {
                    if j != i && self.removal_applies(q, pending[i].source)? {
                        continue 'candidates;
                    }
                }
                pick = Some(i);
                break;
            }
            // CR 613.8b: a loop applies in timestamp order (APNAP for ties).
            let pick = pick.unwrap_or_else(|| {
                ambiguous |= pending.len() > 1
                    && (pending[0].time, pending[0].rank) == (pending[1].time, pending[1].rank)
                    && pending[0].source.id != pending[1].source.id;
                0
            });
            alive.push(pending.remove(pick));
        }
    }

    /// The alive removals that apply to `object`, with their timestamps.
    pub(super) fn removals_on<'a>(
        &'a self,
        alive: &[Removal<'a>],
        object: ObjectId,
    ) -> Result<Vec<(u64, StaticKey)>, String> {
        let Ok(target) = self.current(object) else {
            return Ok(vec![]);
        };
        let mut out = vec![];
        for r in alive {
            if self.removal_applies(r, target)? {
                out.push((r.time, r.key));
            }
        }
        Ok(out)
    }

    /// CR 113.10, 305.7, 613.1f: the object no longer has the abilities of
    /// its card (printed keywords, triggered, activated, mana and static
    /// abilities, and characteristic-defining abilities).
    pub(crate) fn printed_abilities_lost(&self, object: ObjectId) -> bool {
        self.printed_abilities_lost_with_sources(object, None)
            .unwrap_or(false)
    }

    pub(super) fn printed_abilities_lost_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<bool, String> {
        if !self
            .current(object)
            .is_ok_and(|o| o.present_on_battlefield())
        {
            return Ok(false);
        }
        if self.land_type_setters(sources) && self.type_layer(object)?.rules_text_removed {
            return Ok(true);
        }
        let alive = self.alive_removals(sources)?;
        Ok(!self.removals_on(&alive, object)?.is_empty())
    }

    /// CR 305.6, 613.1f: intrinsic land mana abilities come from the layer-4
    /// types, so only a layer-6 removal takes them away.
    pub(super) fn intrinsic_mana_lost(&self, object: ObjectId) -> Result<bool, String> {
        let alive = self.alive_removals(None)?;
        Ok(!self.removals_on(&alive, object)?.is_empty())
    }

    /// Whether the static `key` of `source`, whose effect starts in `first`
    /// (`None` for a rule), still exists (CR 305.7, 613.6, 613.8a).
    pub(super) fn static_exists(
        &self,
        source: &Object,
        key: StaticKey,
        first: Option<Layer>,
        alive: &[Removal<'_>],
        setters: bool,
    ) -> Result<bool, String> {
        if first == Some(Layer::Type) {
            // Decided with its recipients in layer 4.
            return Ok(true);
        }
        if setters && !self.keeps_rules_text(source.id)? {
            return Ok(false);
        }
        match first {
            Some(layer) if layer < Layer::Ability => Ok(true),
            Some(Layer::Ability) => {
                if alive.iter().any(|r| r.key == key) {
                    return Ok(true);
                }
                for r in alive {
                    if r.key != key && self.removal_applies(r, source)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => {
                for r in alive {
                    if self.removal_applies(r, source)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}

impl RulesTxn {
    /// CR 614.12: whether a permanent entering with the copiable values of
    /// `entering` would lack the abilities its card generates once on the
    /// battlefield, so its own entry replacements do not apply. Only a
    /// filtered scope reaches an entering permanent; admitted filters read
    /// copiable card types, supertypes and token status (`kir::statics`).
    pub(super) fn entry_abilities_lost(&self, entering: ObjectId) -> Result<bool, String> {
        let Some(object) = self.state.rules.objects.get(&entering.card) else {
            return Ok(false);
        };
        let probe = Object {
            id: entering,
            tapped: false,
            ..object.clone()
        };
        let printed = &self.copiable_definition(entering)?.characteristics;
        for r in self.alive_removals(None)? {
            if matches!(r.affects, Scope::Matching { .. })
                && self.static_matches(r.affects, r.source, &probe, printed, probe.controller)
            {
                return Ok(true);
            }
        }
        if !printed.has_type(CardKind::Land) || !self.land_type_setters(None) {
            return Ok(false);
        }
        for (source, _, effect) in self.indexed_battlefield_statics(None) {
            if let StaticEffect::Continuous {
                affects: affects @ Scope::Matching { .. },
                changes,
                condition: None,
            } = effect
                && changes
                    .iter()
                    .any(|c| matches!(c, Change::SetLandSubtypes { .. }))
                && self.static_matches(affects, source, &probe, printed, probe.controller)
                && self.keeps_rules_text(source.id)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CR 613.8a: what a land- or creature-subtype setter does depends on
    /// the recipient's card types, so the scheduler compares it.
    #[test]
    fn subtype_setters_take_effect_only_on_matching_types() {
        let mut facts = Characteristics {
            kind: CardKind::Land,
            additional_types: vec![],
            subtypes: vec!["Forest".into()],
            all_creature_types: false,
            basic: false,
            legendary: false,
            snow: false,
            hybrid_mana: vec![],
            mana_cost: ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            },
            colors: vec![],
            color_identity: vec![],
            power: None,
            toughness: None,
            front_face_mana_value: None,
        };
        let mountain = Change::SetLandSubtypes {
            subtypes: vec!["Mountain".into()],
        };
        let elk = Change::SetCreatureSubtypes {
            subtypes: vec!["Elk".into()],
        };
        assert!(change_takes_effect(&mountain, &facts));
        assert!(!change_takes_effect(&elk, &facts));
        let printed = facts.clone();
        apply_type_change(
            &mut facts,
            &Change::SetCardTypes {
                kinds: vec![CardKind::Creature],
            },
            &printed,
            &mut false,
        );
        assert!(facts.subtypes.is_empty() && facts.power.is_some());
        assert!(!change_takes_effect(&mountain, &facts));
        assert!(change_takes_effect(&elk, &facts));
        assert!(!change_takes_effect(
            &Change::RemoveCardTypes {
                kinds: vec![CardKind::Land]
            },
            &facts
        ));
    }
}
