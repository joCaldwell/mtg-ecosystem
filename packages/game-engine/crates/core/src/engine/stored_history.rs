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
struct Row<T> {
    value: T,
    bytes: OnceLock<Result<Box<[u8]>, String>>,
}
impl<T: Clone> Clone for Row<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            bytes: OnceLock::new(),
        }
    }
}
#[derive(Clone)]
pub(super) struct StoredRow<T>(Arc<Row<T>>);
pub(super) type StoredObject = StoredRow<VisibleObject>;
pub(super) type StoredPlayer = StoredRow<VisiblePlayer>;
impl<T: Serialize> StoredRow<T> {
    fn new(value: T) -> Self {
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
impl<T> Deref for StoredRow<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.value
    }
}
impl<T: Clone> DerefMut for StoredRow<T> {
    fn deref_mut(&mut self) -> &mut T {
        let row = Arc::make_mut(&mut self.0);
        row.bytes.take();
        &mut row.value
    }
}
impl<T: Eq> PartialEq for StoredRow<T> {
    fn eq(&self, other: &Self) -> bool {
        // The row type implements Eq. A shared Row therefore proves reflexive
        // value equality; separately allocated rows compare EVERY typed field.
        // The OnceLock is derived bytes, excluded from both sides of equality.
        Arc::ptr_eq(&self.0, &other.0) || self.0.value == other.0.value
    }
}
impl<T: Eq> Eq for StoredRow<T> {}
impl<T: fmt::Debug> fmt::Debug for StoredRow<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.value.fmt(f)
    }
}
impl<T: Serialize> Serialize for StoredRow<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.value.serialize(s)
    }
}
impl<'de, T: Deserialize<'de> + Serialize> Deserialize<'de> for StoredRow<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Self::new)
    }
}

// Persistent bottom-prefix stack. A push allocates one node; a pop retains
// its predecessor. Priority-only views retain the same root. Nodes never cross
// viewers. Full typed prefix equality is required, including order/duplicates.
#[derive(Clone, Default)]
pub(super) struct StoredStack {
    tail: Option<Arc<StackNode>>,
    len: usize,
}
struct StackNode {
    value: VisibleStackEntry,
    previous: StoredStack,
    bytes: OnceLock<Result<Box<[u8]>, String>>,
}
impl Drop for StoredStack {
    fn drop(&mut self) {
        // Deep legal stacks must not recurse through Arc destructors.
        while let Some(tail) = self.tail.take() {
            let Ok(mut node) = Arc::try_unwrap(tail) else {
                break;
            };
            self.tail = node.previous.tail.take();
        }
    }
}
impl StoredStack {
    fn rows(&self) -> Vec<&StackNode> {
        let mut rows = Vec::with_capacity(self.len);
        let mut next = self.tail.as_deref();
        while let Some(node) = next {
            rows.push(node);
            next = node.previous.tail.as_deref();
        }
        rows.reverse();
        rows
    }
    fn from_rows(rows: Vec<VisibleStackEntry>, previous: Option<&Self>) -> Self {
        let old = previous.map(Self::rows).unwrap_or_default();
        let mut stack = Self::default();
        let mut prefix = true;
        for (i, value) in rows.into_iter().enumerate() {
            if prefix && old.get(i).is_some_and(|n| n.value == value) {
                // The next node's predecessor owns precisely this prefix.
                stack = if i + 1 < old.len() {
                    old[i + 1].previous.clone()
                } else {
                    previous.unwrap().clone()
                };
            } else {
                prefix = false;
                stack = Self {
                    len: stack.len + 1,
                    tail: Some(Arc::new(StackNode {
                        value,
                        previous: stack,
                        bytes: OnceLock::new(),
                    })),
                };
            }
        }
        stack
    }
    fn to_public(&self) -> Vec<VisibleStackEntry> {
        self.rows().into_iter().map(|n| n.value.clone()).collect()
    }
    fn matches_public(&self, rows: &[VisibleStackEntry]) -> bool {
        self.len == rows.len() && self.rows().iter().zip(rows).all(|(n, r)| n.value == *r)
    }
    fn write_canonical<W: Write>(&self, writer: &mut W) -> Result<(), String> {
        for (i, row) in self.rows().into_iter().enumerate() {
            if i != 0 {
                writer.write_all(b",").map_err(|e| e.to_string())?;
            }
            let bytes = row
                .bytes
                .get_or_init(|| canonical(&row.value).map(|s| s.into_bytes().into_boxed_slice()));
            writer
                .write_all(bytes.as_deref().map_err(Clone::clone)?)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn node_pointers(&self) -> Vec<usize> {
        self.rows()
            .iter()
            .map(|n| *n as *const StackNode as usize)
            .collect()
    }
}
impl PartialEq for StoredStack {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        let (mut a, mut b) = (&self.tail, &other.tail);
        loop {
            match (a, b) {
                (Some(x), Some(y)) => {
                    if Arc::ptr_eq(x, y) {
                        return true;
                    }
                    if x.value != y.value {
                        return false;
                    }
                    (a, b) = (&x.previous.tail, &y.previous.tail);
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}
impl Eq for StoredStack {}
impl fmt::Debug for StoredStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_public().fmt(f)
    }
}
impl Serialize for StoredStack {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.len))?;
        for row in self.rows() {
            seq.serialize_element(&row.value)?;
        }
        seq.end()
    }
}
impl<'de> Deserialize<'de> for StoredStack {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::<VisibleStackEntry>::deserialize(d).map(|rows| Self::from_rows(rows, None))
    }
}

// Same serialized field order and serde omission rules as public PermittedView;
// exhaustive conversions below force future public fields to be accounted for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredViewData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sacrifice_choices: Vec<(PlayerId, ObjectHandle)>,
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
    #[serde(with = "shared_vec")]
    pub(super) commanders: Arc<Vec<VisibleCommander>>,
    pub(super) priority: Option<PlayerId>,
    #[serde(with = "shared_vec")]
    pub(super) players: Arc<Vec<StoredPlayer>>,
    #[serde(with = "shared_vec")]
    pub(super) objects: Arc<Vec<StoredObject>>,
    pub(super) stack: StoredStack,
}
// Private ownership only: no Arc marker/identity is added to the public array.
// Keep serde's workspace feature set unchanged (no generic rc feature needed).
mod shared_vec {
    use super::*;
    pub(super) fn serialize<T: Serialize, S: Serializer>(
        value: &Arc<Vec<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().serialize(serializer)
    }
    pub(super) fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<Vec<T>>, D::Error> {
        Vec::<T>::deserialize(deserializer).map(Arc::new)
    }
}
#[derive(Clone, Copy)]
enum StreamedRows {
    Commanders,
    Objects,
    Players,
    Stack,
}
struct Segments {
    parts: Vec<(Box<[u8]>, StreamedRows)>,
    after: Box<[u8]>,
}
type CollectionBytes = Arc<OnceLock<Result<Box<[u8]>, String>>>;
pub(super) struct StoredPermittedView {
    data: StoredViewData,
    // Metadata excludes every shared collection. Collection canonical bytes
    // follow the same immutable allocation as their typed values.
    segments: OnceLock<Result<Segments, String>>,
    commanders_bytes: CollectionBytes,
}
impl Clone for StoredPermittedView {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            segments: OnceLock::new(),
            commanders_bytes: self.commanders_bytes.clone(),
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
            commanders_bytes: Arc::new(OnceLock::new()),
        })
    }
}
impl From<PermittedView> for StoredPermittedView {
    fn from(view: PermittedView) -> Self {
        let PermittedView {
            sacrifice_choices,
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
        // Deliberately allocate a fresh narrow row-slot vector. An owning
        // Vec<VisibleObject>::into_iter().map(...).collect::<Vec<StoredObject>>()
        // may reuse the wide source allocation as excess destination capacity
        // (Rust alloc/src/vec/in_place_collect.rs). Row sharing below cannot
        // release that per-view excess capacity. Preserve every value/order.
        let mut stored_objects = Vec::with_capacity(objects.len());
        for object in objects {
            stored_objects.push(StoredObject::new(object));
        }
        let mut stored_players = Vec::with_capacity(players.len());
        for player in players {
            stored_players.push(StoredPlayer::new(player));
        }
        Self {
            data: StoredViewData {
                sacrifice_choices,
                setup,
                effects,
                combat,
                viewer,
                active,
                turn_owner,
                turn,
                live_turn_order,
                commanders: Arc::new(commanders),
                priority,
                players: Arc::new(stored_players),
                objects: Arc::new(stored_objects),
                stack: StoredStack::from_rows(stack, None),
            },
            segments: OnceLock::new(),
            commanders_bytes: Arc::new(OnceLock::new()),
        }
    }
}
impl StoredPermittedView {
    pub(super) fn to_public(&self) -> PermittedView {
        let StoredViewData {
            sacrifice_choices,
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
            sacrifice_choices: sacrifice_choices.clone(),
            setup: setup.clone(),
            effects: effects.clone(),
            combat: combat.clone(),
            viewer: *viewer,
            active: *active,
            turn_owner: *turn_owner,
            turn: turn.clone(),
            live_turn_order: live_turn_order.clone(),
            commanders: commanders.as_ref().clone(),
            priority: *priority,
            players: players.iter().map(|r| r.0.value.clone()).collect(),
            objects: objects.iter().map(|r| r.0.value.clone()).collect(),
            stack: stack.to_public(),
        }
    }
    pub(super) fn matches_public(&self, view: &PermittedView) -> bool {
        let PermittedView {
            sacrifice_choices,
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
            sacrifice_choices: sc,
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
        sc == sacrifice_choices
            && s == setup
            && e == effects
            && c == combat
            && v == viewer
            && a == active
            && o == turn_owner
            && t == turn
            && l == live_turn_order
            && d.as_ref() == commanders
            && p == priority
            && u.len() == players.len()
            && u.iter().zip(players).all(|(r, p)| r.0.value == *p)
            && k.matches_public(stack)
            && r.len() == objects.len()
            && r.iter().zip(objects).all(|(r, o)| r.0.value == *o)
    }
    fn reuse_previous(&mut self, previous: Option<&Self>) {
        let Some(previous) = previous.filter(|p| p.viewer == self.viewer) else {
            return;
        };
        self.data.stack = if self.data.stack == previous.data.stack {
            previous.data.stack.clone()
        } else {
            StoredStack::from_rows(self.data.stack.to_public(), Some(&previous.data.stack))
        };
        // Complete ordered typed equality, never a hash or handle-only test.
        // All shared element types are Eq; pointer identity cannot bypass an
        // irreflexive PartialEq value. Sharing is ONLY this same-viewer predecessor.
        if self.data.players.as_ref() == previous.data.players.as_ref() {
            self.data.players = previous.data.players.clone();
        } else {
            let mut rows = previous
                .players
                .iter()
                .map(|r| (r.player, r))
                .collect::<BTreeMap<_, _>>();
            for row in Arc::make_mut(&mut self.data.players) {
                if let Some(old) = rows.remove(&row.player)
                    && old == row
                {
                    *row = old.clone();
                }
            }
        }
        if self.data.commanders.as_ref() == previous.data.commanders.as_ref() {
            self.data.commanders = previous.data.commanders.clone();
            self.commanders_bytes = previous.commanders_bytes.clone();
        }
        if self.data.objects.as_ref() == previous.data.objects.as_ref() {
            self.data.objects = previous.data.objects.clone();
        } else {
            // Consume each previous handle at most once: malformed duplicate
            // rows still serialize individually, in their original exact order.
            let mut rows = previous
                .objects
                .iter()
                .map(|r| (r.handle.0.clone(), r.clone()))
                .collect::<BTreeMap<_, _>>();
            for row in Arc::make_mut(&mut self.data.objects) {
                if let Some(old) = rows.remove(&row.handle.0)
                    && old == *row
                {
                    *row = old;
                }
            }
        }
    }
    fn metadata_segments(&self) -> Result<&Segments, String> {
        self.segments
            .get_or_init(|| {
                let mut skeleton = self.data.clone();
                skeleton.objects = Arc::new(vec![]);
                skeleton.stack = StoredStack::default();
                skeleton.players = Arc::new(vec![]);
                skeleton.commanders = Arc::new(vec![]);
                let value = serde_json::to_value(&skeleton).map_err(|e| e.to_string())?;
                let map = value.as_object().ok_or("stored View metadata object")?;
                let mut entries = map.iter().collect::<Vec<_>>();
                entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
                let mut out = vec![b'{'];
                let mut parts = vec![];
                for (index, (key, value)) in entries.into_iter().enumerate() {
                    if index != 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut out, key).map_err(|e| e.to_string())?;
                    out.push(b':');
                    let stream = match key.as_str() {
                        "commanders" => Some(StreamedRows::Commanders),
                        "objects" => Some(StreamedRows::Objects),
                        "players" => Some(StreamedRows::Players),
                        "stack" => Some(StreamedRows::Stack),
                        _ => None,
                    };
                    if let Some(stream) = stream {
                        if !value.as_array().is_some_and(Vec::is_empty) {
                            return Err("stored metadata contains shared rows".into());
                        }
                        out.push(b'[');
                        parts.push((std::mem::take(&mut out).into_boxed_slice(), stream));
                        out.push(b']');
                    } else {
                        serde_json::to_writer(&mut out, value).map_err(|e| e.to_string())?;
                    }
                }
                if parts.len() != 4 {
                    return Err("stored metadata missing shared collections".into());
                }
                out.push(b'}');
                Ok(Segments {
                    parts,
                    after: out.into_boxed_slice(),
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub(super) fn write_canonical<W: Write>(&self, writer: &mut W) -> Result<(), String> {
        fn collection<W: Write, T: Serialize>(
            writer: &mut W,
            rows: &[T],
            memo: &CollectionBytes,
        ) -> Result<(), String> {
            let bytes =
                memo.get_or_init(|| canonical(&rows).map(|s| s.into_bytes().into_boxed_slice()));
            let bytes = bytes.as_deref().map_err(Clone::clone)?;
            // Metadata supplies the array brackets, including for empty arrays.
            writer
                .write_all(&bytes[1..bytes.len() - 1])
                .map_err(|e| e.to_string())
        }
        let segments = self.metadata_segments()?;
        for (prefix, rows) in &segments.parts {
            writer.write_all(prefix).map_err(|e| e.to_string())?;
            match rows {
                StreamedRows::Commanders => {
                    collection(writer, self.commanders.as_slice(), &self.commanders_bytes)?
                }
                StreamedRows::Players => {
                    for (index, row) in self.players.iter().enumerate() {
                        if index != 0 {
                            writer.write_all(b",").map_err(|e| e.to_string())?;
                        }
                        writer.write_all(row.bytes()?).map_err(|e| e.to_string())?;
                    }
                }
                StreamedRows::Stack => self.stack.write_canonical(writer)?,
                StreamedRows::Objects => {
                    for (index, row) in self.objects.iter().enumerate() {
                        if index != 0 {
                            writer.write_all(b",").map_err(|e| e.to_string())?;
                        }
                        writer.write_all(row.bytes()?).map_err(|e| e.to_string())?;
                    }
                }
            }
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
        edit(&mut Arc::make_mut(&mut self.data.objects)[index]);
    }
}

// Canonical serialized length is a platform-independent policy unit, not RSS.
// Key sorting changes order but not encoded length; the streaming serde writer
// uses the same JSON escaping/number encoding as canonical(), without a buffer.
// Charge new retained content only. Allocator/memo warmth never selects a fault.
pub(super) fn payload_charge(value: &impl Serialize) -> usize {
    struct Count(usize);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    if serde_json::to_writer(&mut count, value).is_err() {
        return usize::MAX;
    }
    count.0
}
impl StoredPermittedView {
    fn retention_charge(&self, previous: Option<&Self>) -> usize {
        let previous = previous.filter(|p| p.viewer == self.viewer);
        let mut metadata = self.data.clone();
        metadata.objects = Arc::new(vec![]);
        metadata.stack = StoredStack::default();
        metadata.players = Arc::new(vec![]);
        metadata.commanders = Arc::new(vec![]);
        // Fixed policy overhead for the view/history slots and metadata memo.
        // Deliberately independent of target pointer width and Rust layout.
        let mut charge = payload_charge(&metadata).saturating_add(256);
        // New collection slots cost a fixed eight policy bytes each on ALL
        // targets; this is a reference allowance, never size_of::<Arc<_>>().
        if !previous.is_some_and(|p| Arc::ptr_eq(&self.players, &p.players)) {
            charge = charge.saturating_add(self.players.len().saturating_mul(8));
            let mut old = previous
                .map(|p| {
                    p.players
                        .iter()
                        .map(|r| (r.player, r))
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();
            for row in self.players.iter() {
                if !old
                    .remove(&row.player)
                    .is_some_and(|r| Arc::ptr_eq(&r.0, &row.0))
                {
                    charge = charge.saturating_add(payload_charge(&row.0.value));
                }
            }
        }
        if !previous.is_some_and(|p| Arc::ptr_eq(&self.commanders, &p.commanders)) {
            charge = charge.saturating_add(payload_charge(self.commanders.as_ref()));
        }
        if !previous.is_some_and(|p| Arc::ptr_eq(&self.objects, &p.objects)) {
            charge = charge.saturating_add(self.objects.len().saturating_mul(8));
            let old = previous
                .map(|p| {
                    p.objects
                        .iter()
                        .map(|r| (r.handle.0.as_str(), r))
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();
            for row in self.objects.iter() {
                if !old
                    .get(row.handle.0.as_str())
                    .is_some_and(|r| Arc::ptr_eq(&r.0, &row.0))
                {
                    charge = charge.saturating_add(payload_charge(&row.0.value));
                }
            }
        }
        let old = previous.map(|p| p.stack.rows()).unwrap_or_default();
        for (i, row) in self.stack.rows().into_iter().enumerate() {
            if !old.get(i).is_some_and(|r| std::ptr::eq(*r, row)) {
                charge = charge.saturating_add(payload_charge(&row.value));
            }
        }
        charge
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct StoredHistory(History<StoredPermittedView>, usize);
impl PartialEq for StoredHistory {
    fn eq(&self, other: &Self) -> bool {
        // StoredPermittedView is Eq, with complete typed fallback for entries
        // that do not share immutable allocation identity. Public History<T>
        // PartialEq remains available, unchanged, even when T is not Eq.
        self.0.eq_reflexive(&other.0)
    }
}
impl Eq for StoredHistory {}
impl StoredHistory {
    pub(super) fn retention_charge(&self) -> usize {
        self.1
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
        self.1 = 0;
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
        self.1 = self.1.saturating_add(view.retention_charge(self.last()));
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

pub(super) fn is_zero_charge(n: &usize) -> bool {
    *n == 0
}
