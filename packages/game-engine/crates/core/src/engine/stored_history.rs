//! Internal retained observations. All public JSON values/order remain complete.
//! Sharing is previous-view-only, same-viewer, handle+full typed equality.
use super::*;
use serde::{
    Deserializer, Serializer,
    de::{SeqAccess, Visitor},
};
use std::{
    fmt,
    io::Write,
    ops::{Deref, DerefMut, Index, IndexMut},
    sync::OnceLock,
};

#[derive(Debug)]
struct Row {
    value: VisibleObject,
    bytes: OnceLock<Result<Box<[u8]>, String>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            bytes: OnceLock::new(),
        }
    }
}
#[derive(Clone)]
pub(super) struct StoredObject(Arc<Row>);
impl StoredObject {
    fn new(value: VisibleObject) -> Self {
        Self(Arc::new(Row {
            value,
            bytes: OnceLock::new(),
        }))
    }
    fn bytes(&self) -> Result<&[u8], String> {
        self.0
            .bytes
            .get_or_init(|| canonical(&self.0.value).map(|s| s.into_bytes().into_boxed_slice()))
            .as_deref()
            .map_err(Clone::clone)
    }
}
impl Deref for StoredObject {
    type Target = VisibleObject;
    fn deref(&self) -> &VisibleObject {
        &self.0.value
    }
}
impl DerefMut for StoredObject {
    fn deref_mut(&mut self) -> &mut VisibleObject {
        let row = Arc::make_mut(&mut self.0);
        row.bytes.take();
        &mut row.value
    }
}
impl PartialEq for StoredObject {
    fn eq(&self, other: &Self) -> bool {
        self.0.value == other.0.value
    }
}
impl Eq for StoredObject {}
impl fmt::Debug for StoredObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.value.fmt(f)
    }
}
impl Serialize for StoredObject {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.value.serialize(s)
    }
}
impl<'de> Deserialize<'de> for StoredObject {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        VisibleObject::deserialize(d).map(Self::new)
    }
}

// Same field order, types and serde omission rules as public PermittedView;
// exhaustive conversions below force future public fields to be accounted for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredViewData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) setup: Option<VisibleSetup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) effects: Vec<VisibleEffect>,
    pub(super) combat: Vec<VisibleAttacker>,
    pub(super) viewer: PlayerId,
    pub(super) active: Option<PlayerId>,
    pub(super) turn_owner: PlayerId,
    pub(super) turn: TurnState,
    pub(super) live_turn_order: Vec<PlayerId>,
    pub(super) commanders: Vec<VisibleCommander>,
    pub(super) priority: Option<PlayerId>,
    pub(super) players: Vec<VisiblePlayer>,
    pub(super) objects: Vec<StoredObject>,
    pub(super) stack: Vec<VisibleStackEntry>,
}
struct Segments {
    before: Box<[u8]>,
    after: Box<[u8]>,
}
pub(super) struct StoredPermittedView {
    data: StoredViewData,
    // Only metadata/punctuation around objects, never a whole object array.
    segments: OnceLock<Result<Segments, String>>,
}
impl Clone for StoredPermittedView {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            segments: OnceLock::new(),
        }
    }
}
impl Deref for StoredPermittedView {
    type Target = StoredViewData;
    fn deref(&self) -> &StoredViewData {
        &self.data
    }
}
impl PartialEq for StoredPermittedView {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl Eq for StoredPermittedView {}
impl fmt::Debug for StoredPermittedView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}
impl Serialize for StoredPermittedView {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.data.serialize(s)
    }
}
impl<'de> Deserialize<'de> for StoredPermittedView {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        StoredViewData::deserialize(d).map(|data| Self {
            data,
            segments: OnceLock::new(),
        })
    }
}
impl From<PermittedView> for StoredPermittedView {
    fn from(view: PermittedView) -> Self {
        let PermittedView {
            setup,
            effects,
            combat,
            viewer,
            active,
            turn_owner,
            turn,
            live_turn_order,
            commanders,
            priority,
            players,
            objects,
            stack,
        } = view;
        Self {
            data: StoredViewData {
                setup,
                effects,
                combat,
                viewer,
                active,
                turn_owner,
                turn,
                live_turn_order,
                commanders,
                priority,
                players,
                objects: objects.into_iter().map(StoredObject::new).collect(),
                stack,
            },
            segments: OnceLock::new(),
        }
    }
}
impl StoredPermittedView {
    pub(super) fn to_public(&self) -> PermittedView {
        let StoredViewData {
            setup,
            effects,
            combat,
            viewer,
            active,
            turn_owner,
            turn,
            live_turn_order,
            commanders,
            priority,
            players,
            objects,
            stack,
        } = &self.data;
        PermittedView {
            setup: setup.clone(),
            effects: effects.clone(),
            combat: combat.clone(),
            viewer: *viewer,
            active: *active,
            turn_owner: *turn_owner,
            turn: turn.clone(),
            live_turn_order: live_turn_order.clone(),
            commanders: commanders.clone(),
            priority: *priority,
            players: players.clone(),
            objects: objects.iter().map(|r| r.0.value.clone()).collect(),
            stack: stack.clone(),
        }
    }
    pub(super) fn matches_public(&self, view: &PermittedView) -> bool {
        let PermittedView {
            setup,
            effects,
            combat,
            viewer,
            active,
            turn_owner,
            turn,
            live_turn_order,
            commanders,
            priority,
            players,
            objects,
            stack,
        } = view;
        let StoredViewData {
            setup: s,
            effects: e,
            combat: c,
            viewer: v,
            active: a,
            turn_owner: o,
            turn: t,
            live_turn_order: l,
            commanders: d,
            priority: p,
            players: u,
            objects: r,
            stack: k,
        } = &self.data;
        s == setup
            && e == effects
            && c == combat
            && v == viewer
            && a == active
            && o == turn_owner
            && t == turn
            && l == live_turn_order
            && d == commanders
            && p == priority
            && u == players
            && k == stack
            && r.len() == objects.len()
            && r.iter().zip(objects).all(|(r, o)| r.0.value == *o)
    }
    fn reuse_previous(&mut self, previous: Option<&Self>) {
        let Some(previous) = previous.filter(|p| p.viewer == self.viewer) else {
            return;
        };
        // Consume each previous handle at most once: malformed duplicate rows
        // still serialize individually, in their original exact order.
        let mut rows = previous
            .objects
            .iter()
            .map(|r| (r.handle.0.clone(), r.clone()))
            .collect::<BTreeMap<_, _>>();
        for row in &mut self.data.objects {
            if let Some(old) = rows.remove(&row.handle.0)
                && old == *row
            {
                *row = old;
            }
        }
    }
    fn metadata_segments(&self) -> Result<&Segments, String> {
        self.segments
            .get_or_init(|| {
                let StoredViewData {
                    setup,
                    effects,
                    combat,
                    viewer,
                    active,
                    turn_owner,
                    turn,
                    live_turn_order,
                    commanders,
                    priority,
                    players,
                    objects: _,
                    stack,
                } = &self.data;
                let skeleton = PermittedView {
                    setup: setup.clone(),
                    effects: effects.clone(),
                    combat: combat.clone(),
                    viewer: *viewer,
                    active: *active,
                    turn_owner: *turn_owner,
                    turn: turn.clone(),
                    live_turn_order: live_turn_order.clone(),
                    commanders: commanders.clone(),
                    priority: *priority,
                    players: players.clone(),
                    objects: vec![],
                    stack: stack.clone(),
                };
                let value = serde_json::to_value(skeleton).map_err(|e| e.to_string())?;
                let map = value.as_object().ok_or("stored View metadata object")?;
                let mut entries = map.iter().collect::<Vec<_>>();
                entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
                let (mut before, mut after) = (vec![b'{'], vec![]);
                let mut seen = false;
                for (index, (key, value)) in entries.into_iter().enumerate() {
                    let out = if seen { &mut after } else { &mut before };
                    if index != 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut *out, key).map_err(|e| e.to_string())?;
                    out.push(b':');
                    if key == "objects" {
                        if !value.as_array().is_some_and(Vec::is_empty) {
                            return Err("stored metadata contains object rows".into());
                        }
                        out.push(b'[');
                        seen = true;
                        after.push(b']');
                    } else {
                        serde_json::to_writer(&mut *out, value).map_err(|e| e.to_string())?;
                    }
                }
                if !seen {
                    return Err("stored metadata missing objects".into());
                }
                after.push(b'}');
                Ok(Segments {
                    before: before.into_boxed_slice(),
                    after: after.into_boxed_slice(),
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub(super) fn write_canonical<W: Write>(&self, writer: &mut W) -> Result<(), String> {
        let segments = self.metadata_segments()?;
        writer
            .write_all(&segments.before)
            .map_err(|e| e.to_string())?;
        for (index, row) in self.data.objects.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",").map_err(|e| e.to_string())?;
            }
            writer.write_all(row.bytes()?).map_err(|e| e.to_string())?;
        }
        writer.write_all(&segments.after).map_err(|e| e.to_string())
    }
    /// Rare deliberate historical edits are explicit. A mutable public value
    /// is detached from caches before exposure; even panic leaves self intact.
    /// Reintern only against this SAME viewer, after the complete edit.
    #[cfg(test)]
    pub(super) fn update_public(&mut self, edit: impl FnOnce(&mut PermittedView)) {
        let mut public = self.to_public();
        edit(&mut public);
        let mut replacement = Self::from(public);
        replacement.reuse_previous(Some(self));
        *self = replacement;
    }
    #[cfg(test)]
    pub(super) fn replace_public_objects(&mut self, objects: Vec<VisibleObject>) {
        self.update_public(|v| v.objects = objects);
    }
    #[cfg(test)]
    pub(super) fn mutate_object(&mut self, index: usize, edit: impl FnOnce(&mut VisibleObject)) {
        // No &mut StoredObject/StoredViewData escapes; the caller cannot warm a
        // row cache while retaining its mutable underlying VisibleObject.
        edit(&mut self.data.objects[index]);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(super) struct StoredHistory(History<StoredPermittedView>);
impl StoredHistory {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
    pub(super) fn last(&self) -> Option<&StoredPermittedView> {
        self.0.last()
    }
    pub(super) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = &StoredPermittedView> + ExactSizeIterator {
        self.0.iter()
    }
    pub(super) fn push(&mut self, mut view: StoredPermittedView) {
        view.reuse_previous(self.last());
        self.0.push(view);
    }
    pub(super) fn push_public(&mut self, view: PermittedView) {
        self.push(view.into());
    }
    #[cfg(test)]
    pub(super) fn canonical_entry_bytes(&self, index: usize) -> Result<Vec<u8>, String> {
        let mut bytes = vec![];
        self[index].write_canonical(&mut bytes)?;
        Ok(bytes)
    }
}
impl Index<usize> for StoredHistory {
    type Output = StoredPermittedView;
    fn index(&self, index: usize) -> &StoredPermittedView {
        &self.0[index]
    }
}
impl IndexMut<usize> for StoredHistory {
    fn index_mut(&mut self, index: usize) -> &mut StoredPermittedView {
        &mut self.0[index]
    }
}
impl Serialize for StoredHistory {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for StoredHistory {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = StoredHistory;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("complete ordered permitted history array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut history = StoredHistory::default();
                // One view is decoded directly into per-row Arc wrappers; no
                // Vec<PermittedView> or second full public-history buffer.
                // Once viewer is known, compact only against its immediate predecessor.
                while let Some(view) = seq.next_element::<StoredPermittedView>()? {
                    history.push(view);
                }
                Ok(history)
            }
        }
        d.deserialize_seq(Entries)
    }
}

#[cfg(test)]
#[path = "stored_history_tests.rs"]
mod tests;
