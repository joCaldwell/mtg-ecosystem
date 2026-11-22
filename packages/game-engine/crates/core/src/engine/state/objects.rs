//! The object table: one row per physical card, dense by `CardId`.
//!
//! Rows live in fixed chunks behind `Arc`, so a staged fork shares every
//! chunk and a write copies only the chunk it touches. A reader holding the
//! chunks of an earlier state can tell exactly which rows may have changed by
//! comparing chunk identities. The wire form, equality and iteration order
//! are those of the ordered `CardId -> Object` map this replaces.
use crate::engine::{CardId, Object};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, Visitor},
    ser::SerializeMap,
};
use std::{
    fmt,
    ops::{Bound, Index, RangeBounds},
    sync::Arc,
};

/// Rows per chunk. A write clones one chunk of this many rows.
pub const OBJECT_CHUNK: usize = 32;

type Chunk = [Option<Object>; OBJECT_CHUNK];

/// Chunks in ascending order of their first card index, without gaps for
/// unoccupied index ranges: physical cards are dense from zero and tokens are
/// dense from their own base, so both runs stay compact.
#[derive(Clone, Default)]
pub struct ObjectTable {
    chunks: Vec<(u32, Arc<Chunk>)>,
    len: usize,
}

fn slot(card: CardId) -> (u32, usize) {
    (card.0 / OBJECT_CHUNK as u32, card.0 as usize % OBJECT_CHUNK)
}

impl ObjectTable {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Position of the chunk numbered `chunk`, or where it would be inserted.
    fn position(&self, chunk: u32) -> Result<usize, usize> {
        // The run of physical cards starts at zero, so its chunk number is
        // its position.
        match self.chunks.get(chunk as usize) {
            Some((number, _)) if *number == chunk => Ok(chunk as usize),
            _ => self
                .chunks
                .binary_search_by_key(&chunk, |(number, _)| *number),
        }
    }
    pub fn get(&self, card: &CardId) -> Option<&Object> {
        let (chunk, offset) = slot(*card);
        self.chunks[self.position(chunk).ok()?].1[offset].as_ref()
    }
    pub fn contains_key(&self, card: &CardId) -> bool {
        self.get(card).is_some()
    }
    /// Exclusive access to one row. Unshares that row's chunk only.
    pub fn get_mut(&mut self, card: &CardId) -> Option<&mut Object> {
        let (chunk, offset) = slot(*card);
        let position = self.position(chunk).ok()?;
        let chunk = &mut self.chunks[position].1;
        // An absent row must not unshare its chunk.
        chunk[offset].as_ref()?;
        Arc::make_mut(chunk)[offset].as_mut()
    }
    pub fn insert(&mut self, card: CardId, object: Object) -> Option<Object> {
        let (chunk, offset) = slot(card);
        let position = match self.position(chunk) {
            Ok(position) => position,
            Err(position) => {
                let empty: Chunk = std::array::from_fn(|_| None);
                self.chunks.insert(position, (chunk, Arc::new(empty)));
                position
            }
        };
        let previous = Arc::make_mut(&mut self.chunks[position].1)[offset].replace(object);
        if previous.is_none() {
            self.len += 1;
        }
        previous
    }
    pub fn remove(&mut self, card: &CardId) -> Option<Object> {
        let (chunk, offset) = slot(*card);
        let position = self.position(chunk).ok()?;
        let chunk = &mut self.chunks[position].1;
        chunk[offset].as_ref()?;
        let previous = Arc::make_mut(chunk)[offset].take();
        self.len -= 1;
        previous
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    fn rows(&self, front: usize, back: usize) -> Iter<'_> {
        Iter {
            chunks: &self.chunks,
            front,
            back: back.max(front),
        }
    }
    /// Rows in ascending `CardId` order.
    pub fn iter(&self) -> Iter<'_> {
        self.rows(0, self.chunks.len() * OBJECT_CHUNK)
    }
    pub fn keys(&self) -> Keys<'_> {
        Keys(self.iter())
    }
    pub fn values(&self) -> Values<'_> {
        Values(self.iter())
    }
    /// Exclusive access to every row. Unshares every occupied chunk.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Object> {
        self.chunks
            .iter_mut()
            .filter(|(_, chunk)| chunk.iter().any(Option::is_some))
            .flat_map(|(_, chunk)| Arc::make_mut(chunk).iter_mut().filter_map(Option::as_mut))
    }
    pub fn first_key_value(&self) -> Option<(CardId, &Object)> {
        self.iter().next()
    }
    /// The cursor position of the first row whose card index is at least
    /// `index` (a 33-bit value, so one past the last card is representable).
    fn cursor(&self, index: u64) -> usize {
        let chunk = index / OBJECT_CHUNK as u64;
        let position = self
            .chunks
            .partition_point(|(number, _)| u64::from(*number) < chunk);
        match self.chunks.get(position) {
            Some((number, _)) if u64::from(*number) == chunk => {
                position * OBJECT_CHUNK + (index % OBJECT_CHUNK as u64) as usize
            }
            _ => position * OBJECT_CHUNK,
        }
    }
    /// Rows whose `CardId` lies in `range`, ascending.
    pub fn range(&self, range: impl RangeBounds<CardId>) -> Iter<'_> {
        let start = match range.start_bound() {
            Bound::Included(card) => u64::from(card.0),
            Bound::Excluded(card) => u64::from(card.0) + 1,
            Bound::Unbounded => 0,
        };
        let stop = match range.end_bound() {
            Bound::Included(card) => u64::from(card.0) + 1,
            Bound::Excluded(card) => u64::from(card.0),
            Bound::Unbounded => u64::from(u32::MAX) + 1,
        };
        self.rows(self.cursor(start), self.cursor(stop))
    }
    /// Changed card keys, including retired rows. Shared chunks require no row reads.
    pub(in crate::engine) fn changed_cards(
        &self,
        before: &Self,
    ) -> std::collections::BTreeSet<CardId> {
        let mut changed = std::collections::BTreeSet::new();
        for (number, chunk) in &self.chunks {
            let old = before.position(*number).ok().map(|i| &before.chunks[i].1);
            if old.is_some_and(|old| Arc::ptr_eq(chunk, old)) {
                continue;
            }
            for (offset, row) in chunk.iter().enumerate() {
                if row.as_ref() != old.and_then(|old| old[offset].as_ref()) {
                    changed.insert(CardId(number * OBJECT_CHUNK as u32 + offset as u32));
                }
            }
        }
        for (number, old) in &before.chunks {
            if self.position(*number).is_err() {
                for (offset, row) in old.iter().enumerate() {
                    if row.is_some() {
                        changed.insert(CardId(number * OBJECT_CHUNK as u32 + offset as u32));
                    }
                }
            }
        }
        changed
    }

    /// True when both tables share every chunk: no row can differ.
    pub fn shares_rows(&self, other: &Self) -> bool {
        self.chunks.len() == other.chunks.len()
            && self
                .chunks
                .iter()
                .zip(&other.chunks)
                .all(|(a, b)| a.0 == b.0 && Arc::ptr_eq(&a.1, &b.1))
    }
}

/// Occupied rows in ascending `CardId` order. A plain cursor with no
/// destructor, so a loop may return through `&mut` access to the game.
#[derive(Clone)]
pub struct Iter<'a> {
    chunks: &'a [(u32, Arc<Chunk>)],
    front: usize,
    back: usize,
}
impl<'a> Iter<'a> {
    fn row(&self, cursor: usize) -> Option<(CardId, &'a Object)> {
        let (number, chunk): &'a (u32, Arc<Chunk>) = &self.chunks[cursor / OBJECT_CHUNK];
        let offset = cursor % OBJECT_CHUNK;
        // Rows exist only at indices that came from a `CardId`.
        let card = CardId(number * OBJECT_CHUNK as u32 + offset as u32);
        Some((card, chunk[offset].as_ref()?))
    }
}
impl<'a> Iterator for Iter<'a> {
    type Item = (CardId, &'a Object);
    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let cursor = self.front;
            self.front += 1;
            if let Some(row) = self.row(cursor) {
                return Some(row);
            }
        }
        None
    }
}
impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            self.back -= 1;
            if let Some(row) = self.row(self.back) {
                return Some(row);
            }
        }
        None
    }
}
#[derive(Clone)]
pub struct Keys<'a>(Iter<'a>);
impl Iterator for Keys<'_> {
    type Item = CardId;
    fn next(&mut self) -> Option<CardId> {
        self.0.next().map(|(card, _)| card)
    }
}
impl DoubleEndedIterator for Keys<'_> {
    fn next_back(&mut self) -> Option<CardId> {
        self.0.next_back().map(|(card, _)| card)
    }
}
#[derive(Clone)]
pub struct Values<'a>(Iter<'a>);
impl<'a> Iterator for Values<'a> {
    type Item = &'a Object;
    fn next(&mut self) -> Option<&'a Object> {
        self.0.next().map(|(_, object)| object)
    }
}
impl DoubleEndedIterator for Values<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, object)| object)
    }
}

impl Index<&CardId> for ObjectTable {
    type Output = Object;
    fn index(&self, card: &CardId) -> &Object {
        self.get(card).expect("no object row for this card")
    }
}

impl<'a> IntoIterator for &'a ObjectTable {
    type Item = (CardId, &'a Object);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl FromIterator<(CardId, Object)> for ObjectTable {
    fn from_iter<I: IntoIterator<Item = (CardId, Object)>>(rows: I) -> Self {
        let mut table = Self::default();
        for (card, object) in rows {
            table.insert(card, object);
        }
        table
    }
}

impl PartialEq for ObjectTable {
    fn eq(&self, other: &Self) -> bool {
        // An emptied chunk is not a row: compare the occupied rows.
        self.len == other.len
            && (self.shares_rows(other)
                || self
                    .iter()
                    .zip(other.iter())
                    .all(|(a, b)| a.0 == b.0 && a.1 == b.1))
    }
}
impl Eq for ObjectTable {}

impl fmt::Debug for ObjectTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl Serialize for ObjectTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.len))?;
        for (card, object) in self.iter() {
            map.serialize_entry(&card, object)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ObjectTable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Rows;
        impl<'de> Visitor<'de> for Rows {
            type Value = ObjectTable;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map from card to object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ObjectTable, A::Error> {
                let mut table = ObjectTable::default();
                // As for the ordered map this replaces, a later duplicate key
                // replaces the earlier row.
                while let Some((card, object)) = map.next_entry::<CardId, Object>()? {
                    table.insert(card, object);
                }
                Ok(table)
            }
        }
        deserializer.deserialize_map(Rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ObjectId, PlayerId, Zone};
    use mtg_contracts::Quantity;
    use std::collections::BTreeMap;

    fn object(card: u32, incarnation: u32) -> Object {
        let json = serde_json::json!({
            "marked_damage": Quantity::zero(),
            "attack_ready": false,
            "id": ObjectId { card: CardId(card), incarnation },
            "controller": PlayerId(0),
            "default_controller": PlayerId(0),
            "zone": Zone::Exile,
            "tapped": false,
        });
        serde_json::from_value(json).unwrap()
    }

    /// Physical cards near zero, tokens from their base and both `u32` ends.
    const CARDS: [u32; 12] = [
        0,
        1,
        31,
        32,
        33,
        399,
        9_999,
        1 << 31,
        (1 << 31) + 1,
        (1 << 31) + 64,
        u32::MAX - 1,
        u32::MAX,
    ];

    fn check(table: &ObjectTable, model: &BTreeMap<CardId, Object>) {
        assert_eq!(table.len(), model.len());
        assert_eq!(table.is_empty(), model.is_empty());
        let rows: Vec<_> = table.iter().map(|(card, o)| (card, o.clone())).collect();
        let expected: Vec<_> = model.iter().map(|(card, o)| (*card, o.clone())).collect();
        assert_eq!(rows, expected);
        assert!(
            table
                .iter()
                .rev()
                .eq(model.iter().rev().map(|(c, o)| (*c, o)))
        );
        assert!(table.keys().eq(model.keys().copied()));
        assert!(table.values().eq(model.values()));
        assert_eq!(
            table.first_key_value(),
            model.first_key_value().map(|(c, o)| (*c, o))
        );
        for card in CARDS.into_iter().chain([2, 30, 34, 1 << 30]) {
            let card = CardId(card);
            assert_eq!(table.get(&card), model.get(&card));
            assert_eq!(table.contains_key(&card), model.contains_key(&card));
            for bounds in [
                (Bound::Included(card), Bound::Unbounded),
                (Bound::Excluded(card), Bound::Unbounded),
                (Bound::Unbounded, Bound::Included(card)),
                (Bound::Unbounded, Bound::Excluded(card)),
                (Bound::Included(CardId(1)), Bound::Excluded(card)),
            ] {
                if matches!(bounds, (Bound::Included(a), Bound::Excluded(b)) if a > b) {
                    continue;
                }
                assert!(
                    table
                        .range(bounds)
                        .eq(model.range(bounds).map(|(c, o)| (*c, o))),
                    "{bounds:?}"
                );
            }
        }
        // The wire form is the ordered map's own.
        assert_eq!(
            serde_json::to_string(table).unwrap(),
            serde_json::to_string(model).unwrap()
        );
        let decoded: ObjectTable =
            serde_json::from_str(&serde_json::to_string(model).unwrap()).unwrap();
        assert_eq!(&decoded, table);
        assert_eq!(format!("{table:?}"), format!("{model:?}"));
    }

    #[test]
    fn object_table_matches_the_ordered_map_it_replaces() {
        let mut table = ObjectTable::default();
        let mut model = BTreeMap::new();
        check(&table, &model);
        // Insert out of order, replace, mutate and remove.
        for (step, card) in CARDS.into_iter().rev().enumerate() {
            let row = object(card, step as u32);
            assert_eq!(
                table.insert(CardId(card), row.clone()),
                model.insert(CardId(card), row)
            );
            check(&table, &model);
        }
        let replacement = object(32, 77);
        assert_eq!(
            table.insert(CardId(32), replacement.clone()),
            model.insert(CardId(32), replacement)
        );
        table.get_mut(&CardId(399)).unwrap().tapped = true;
        model.get_mut(&CardId(399)).unwrap().tapped = true;
        assert!(table.get_mut(&CardId(2)).is_none());
        for row in table.values_mut() {
            row.attack_ready = true;
        }
        for row in model.values_mut() {
            row.attack_ready = true;
        }
        check(&table, &model);
        for card in [0, 33, 1 << 31, u32::MAX, 5] {
            assert_eq!(table.remove(&CardId(card)), model.remove(&CardId(card)));
            check(&table, &model);
        }
        // A key need not equal the row's own identity; validation decides.
        let moved = table.remove(&CardId(1)).unwrap();
        model.remove(&CardId(1));
        table.insert(CardId(99), moved.clone());
        model.insert(CardId(99), moved);
        check(&table, &model);
        table.clear();
        model.clear();
        check(&table, &model);
    }

    #[test]
    fn object_table_write_unshares_only_the_written_chunk() {
        let base: ObjectTable = (0..100)
            .map(|card| (CardId(card), object(card, 0)))
            .collect();
        let mut fork = base.clone();
        assert!(fork.shares_rows(&base));
        // Reads and absent rows share everything.
        assert!(fork.get_mut(&CardId(500)).is_none());
        assert!(fork.remove(&CardId(500)).is_none());
        assert!(fork.shares_rows(&base));
        fork.get_mut(&CardId(40)).unwrap().tapped = true;
        assert!(!fork.shares_rows(&base));
        assert_ne!(fork, base);
        let shared = |a: &ObjectTable, b: &ObjectTable| {
            a.chunks
                .iter()
                .zip(&b.chunks)
                .map(|(x, y)| Arc::ptr_eq(&x.1, &y.1))
                .collect::<Vec<_>>()
        };
        assert_eq!(shared(&fork, &base), [true, false, true, true]);
        // The base is untouched, and equal content compares equal unshared.
        assert!(!base.get(&CardId(40)).unwrap().tapped);
        fork.get_mut(&CardId(40)).unwrap().tapped = false;
        assert_eq!(fork, base);
        assert!(!fork.shares_rows(&base));
    }
}
