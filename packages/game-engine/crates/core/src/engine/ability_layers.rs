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
}

/// Cross-object recursion is bounded: each nested level excludes one more
/// static from consideration.
const LAYER_DEPTH: usize = 16;

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
        Change::AddCreatureSubtypes { subtypes } => {
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

enum Row<'a> {
    Resolved(&'a Animation),
    Static {
        key: StaticKey,
        source: &'a Object,
        affects: &'a Scope,
        changes: &'a [Change],
    },
}

impl Row<'_> {
    fn apply(&self, facts: &mut Characteristics, printed: &Characteristics, removed: &mut bool) {
        match self {
            Row::Resolved(animation) => apply_animation(facts, animation, printed),
            Row::Static { changes, .. } => {
                for change in changes.iter().filter(|c| c.layer() == Layer::Type) {
                    apply_type_change(facts, change, printed, removed);
                }
            }
        }
    }
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

    fn any_static_change(&self, sources: Option<&[CardId]>, pred: impl Fn(&Change) -> bool) -> bool {
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
        let setters = self.land_type_setters(None);
        self.type_layer_excluding(object, &[], setters)
    }

    /// Whether `source` keeps its rules-text abilities for the effect `key`
    /// (CR 305.7, 613.8a: the effect is not removed by itself).
    pub(super) fn keeps_rules_text(
        &self,
        source: ObjectId,
        key: StaticKey,
        exclude: &[StaticKey],
        setters: bool,
    ) -> Result<bool, String> {
        if !setters {
            return Ok(true);
        }
        // CR 305.7 reaches only a land: a source that is no land card can
        // become one only through a static that sets card types.
        if !self
            .copiable_definition(source)?
            .characteristics
            .has_type(CardKind::Land)
            && !self.any_static_change(None, |c| {
                matches!(c, Change::SetCardTypes { kinds } if kinds.contains(&CardKind::Land))
            })
        {
            return Ok(true);
        }
        let mut excluded = exclude.to_vec();
        excluded.push(key);
        Ok(!self
            .type_layer_excluding(source, &excluded, setters)?
            .rules_text_removed)
    }

    fn type_layer_excluding(
        &self,
        object: ObjectId,
        exclude: &[StaticKey],
        setters: bool,
    ) -> Result<TypeLayer<'_>, String> {
        if exclude.len() > LAYER_DEPTH {
            return Err("layer-4 dependency depth exceeded".into());
        }
        let printed = &self.copiable_definition(object)?.characteristics;
        let mut out = TypeLayer {
            facts: Cow::Borrowed(printed),
            rules_text_removed: false,
            applied: vec![],
        };
        let Some(target) = self
            .current(object)
            .ok()
            .filter(|o| o.present_on_battlefield())
        else {
            return Ok(out);
        };
        let mut rows: Vec<(u64, u8, Row<'_>)> = self
            .resolved_characteristic_sets(object)
            .filter_map(|(time, values)| Some((time, 0, Row::Resolved(values.animation.as_ref()?))))
            .collect();
        for (source, key, effect) in self.indexed_battlefield_statics(None) {
            let StaticEffect::Continuous {
                affects,
                changes,
                condition,
            } = effect
            else {
                continue;
            };
            if first_layer(changes) != Some(Layer::Type) || exclude.contains(&key) {
                continue;
            }
            let possible = match affects {
                Scope::Source => source.id == object,
                Scope::Attachment { .. } => source.attached_to == Some(object),
                Scope::Matching { .. } => true,
            };
            if !possible
                || condition
                    .as_ref()
                    .is_some_and(|c| !self.static_condition(source, c))
                || !self.keeps_rules_text(source.id, key, exclude, setters)?
            {
                continue;
            }
            rows.push((
                source.keyword_timestamp.unwrap_or(0),
                1,
                Row::Static {
                    key,
                    source,
                    affects,
                    changes,
                },
            ));
        }
        if rows.is_empty() {
            return Ok(out);
        }
        // CR 613.7b: creation order, a resolved animation before a static
        // ability with the same timestamp.
        rows.sort_by_key(|(time, order, _)| (*time, *order));
        let member = |row: &Row<'_>, facts: &Characteristics| match row {
            Row::Resolved(_) => true,
            Row::Static {
                source, affects, ..
            } => self.static_matches(affects, source, target, facts, target.controller),
        };
        // CR 613.8b-c: apply the earliest effect that no remaining effect
        // would change the applicability of, re-evaluated after each one;
        // in a loop, timestamp order.
        while !rows.is_empty() {
            let facts = out.facts.as_ref();
            let pick = (0..rows.len())
                .find(|&i| {
                    let row = &rows[i].2;
                    if matches!(
                        row,
                        Row::Resolved(_)
                            | Row::Static {
                                affects: Scope::Source,
                                ..
                            }
                    ) {
                        return true;
                    }
                    let now = member(row, facts);
                    rows.iter().enumerate().all(|(j, (_, _, other))| {
                        if j == i || !member(other, facts) {
                            return true;
                        }
                        let mut after = facts.clone();
                        other.apply(&mut after, printed, &mut false);
                        member(row, &after) == now
                    })
                })
                .unwrap_or(0);
            let (_, _, row) = rows.remove(pick);
            if member(&row, out.facts.as_ref()) {
                row.apply(out.facts.to_mut(), printed, &mut out.rules_text_removed);
                if let Row::Static { key, .. } = row {
                    out.applied.push(key);
                }
            }
        }
        Ok(out)
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
        if !self.ability_removers(sources) {
            return Ok(vec![]);
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
            };
            if first == Layer::Type {
                // Its existence and recipients are fixed in layer 4.
                alive.push(removal);
            } else if self.keeps_rules_text(source.id, key, &[], setters)? {
                if first < Layer::Ability {
                    alive.push(removal);
                } else {
                    pending.push(removal);
                }
            }
        }
        pending.sort_by_key(|r| r.time);
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
                return Ok(alive);
            }
            let mut pick = 0;
            'candidates: for i in 0..pending.len() {
                for (j, q) in pending.iter().enumerate() {
                    if j != i && self.removal_applies(q, pending[i].source)? {
                        continue 'candidates;
                    }
                }
                pick = i;
                break;
            }
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
        if !self.keeps_rules_text(source.id, key, &[], setters)? {
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
        for (source, key, effect) in self.indexed_battlefield_statics(None) {
            if let StaticEffect::Continuous {
                affects: affects @ Scope::Matching { .. },
                changes,
                condition: None,
            } = effect
                && changes
                    .iter()
                    .any(|c| matches!(c, Change::SetLandSubtypes { .. }))
                && self.static_matches(affects, source, &probe, printed, probe.controller)
                && self.keeps_rules_text(source.id, key, &[], true)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
