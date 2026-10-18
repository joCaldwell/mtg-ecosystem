//! Append-only histories share immutable entries and chunks across staging/forks.
//! Their wire format remains an ordinary ordered JSON array.
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};
use std::{
    fmt,
    ops::{Index, IndexMut},
    sync::{Arc, OnceLock},
};

const CHUNK: usize = 32;

// The memo is derived from an immutable entry, never part of history's wire
// format/equality. Complete-entry memo helpers remain only for generic tests;
// production retained views use row/metadata segments in StoredHistory.
// No cache is consulted by Serialize or ordinary iteration.
struct HistoryEntry<T> {
    value: T,
    canonical: OnceLock<Result<Box<[u8]>, String>>,
}
impl<T: Clone> Clone for HistoryEntry<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            canonical: OnceLock::new(),
        }
    }
}
pub struct History<T> {
    chunks: Vec<Arc<Vec<Arc<HistoryEntry<T>>>>>,
    tail: Vec<Arc<HistoryEntry<T>>>,
    len: usize,
    flat: OnceLock<Vec<T>>,
}
impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            chunks: vec![],
            tail: vec![],
            len: 0,
            flat: OnceLock::new(),
        }
    }
}
impl<T: Clone> Clone for History<T> {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            tail: self.tail.clone(),
            len: self.len,
            flat: OnceLock::new(),
        }
    }
}
impl<T> History<T> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn push(&mut self, value: T) {
        self.flat.take();
        self.tail.push(Arc::new(HistoryEntry {
            value,
            canonical: OnceLock::new(),
        }));
        self.len = self
            .len
            .checked_add(1)
            .expect("history address-space limit");
        if self.tail.len() == CHUNK {
            self.chunks.push(Arc::new(std::mem::take(&mut self.tail)));
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn last(&self) -> Option<&T> {
        self.len.checked_sub(1).map(|i| &self[i])
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        (0..self.len).map(|i| &self[i])
    }
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|v| v == value)
    }
    /// Materialize the native flat compatibility view only when requested.
    pub fn as_slice(&self) -> &[T]
    where
        T: Clone,
    {
        self.flat.get_or_init(|| self.iter().cloned().collect())
    }
}
impl<T> History<T> {
    fn entry(&self, index: usize) -> &HistoryEntry<T> {
        assert!(index < self.len);
        let chunk = index / CHUNK;
        if chunk < self.chunks.len() {
            &self.chunks[chunk][index % CHUNK]
        } else {
            &self.tail[index % CHUNK]
        }
    }
    /// Memoize complete canonical bytes for an immutable typed entry. The
    /// private initializer must emit the generic canonical oracle's exact
    /// bytes. A warm hit skips it entirely, including its local accelerator.
    #[cfg(test)]
    pub(super) fn canonical_entry_bytes_with(
        &self,
        index: usize,
        initialize: impl FnOnce(&T) -> Result<Box<[u8]>, String>,
    ) -> Result<&[u8], String> {
        let entry = self.entry(index);
        entry
            .canonical
            .get_or_init(|| initialize(&entry.value))
            .as_deref()
            .map_err(Clone::clone)
    }
    /// Independent generic oracle for initializer and invalidation tests.
    #[cfg(test)]
    pub(super) fn canonical_entry_bytes(&self, index: usize) -> Result<&[u8], String>
    where
        T: Serialize,
    {
        self.canonical_entry_bytes_with(index, |value| {
            mtg_contracts::canonical(value).map(|text| text.into_bytes().into_boxed_slice())
        })
    }
}
impl<T> Index<usize> for History<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.entry(index).value
    }
}
impl<T: Clone> IndexMut<usize> for History<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len);
        self.flat.take();
        let chunk = index / CHUNK;
        let entry = if chunk < self.chunks.len() {
            Arc::make_mut(&mut Arc::make_mut(&mut self.chunks[chunk])[index % CHUNK])
        } else {
            Arc::make_mut(&mut self.tail[index % CHUNK])
        };
        // Arc::make_mut clones a shared entry with an empty memo; unique warm
        // entries still require explicit invalidation BEFORE exposing &mut T.
        entry.canonical.take();
        &mut entry.value
    }
}
impl<T: PartialEq> PartialEq for History<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}
impl<T: Eq> Eq for History<T> {}
impl<T: fmt::Debug> fmt::Debug for History<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<T: Serialize> Serialize for History<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for value in self.iter() {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for History<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Entries<T> {
            type Value = History<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("ordered history array")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut history = History::default();
                while let Some(value) = seq.next_element::<T>()? {
                    history.push(value);
                }
                Ok(history)
            }
        }
        deserializer.deserialize_seq(Entries(std::marker::PhantomData))
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
