//! Append-only histories share immutable entries and chunks across staging/forks.
//! Their wire format remains an ordinary ordered JSON array.
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};
use std::{
    fmt,
    ops::{Index, IndexMut},
    sync::{Arc, OnceLock},
};

const CHUNK: usize = 32;

pub struct History<T> {
    chunks: Vec<Arc<Vec<Arc<T>>>>,
    tail: Vec<Arc<T>>,
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
        self.tail.push(Arc::new(value));
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
impl<T> Index<usize> for History<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        assert!(index < self.len);
        let chunk = index / CHUNK;
        if chunk < self.chunks.len() {
            &self.chunks[chunk][index % CHUNK]
        } else {
            &self.tail[index % CHUNK]
        }
    }
}
impl<T: Clone> IndexMut<usize> for History<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len);
        self.flat.take();
        let chunk = index / CHUNK;
        if chunk < self.chunks.len() {
            Arc::make_mut(&mut Arc::make_mut(&mut self.chunks[chunk])[index % CHUNK])
        } else {
            Arc::make_mut(&mut self.tail[index % CHUNK])
        }
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
        let values = Vec::<T>::deserialize(deserializer)?;
        let mut history = Self::default();
        for value in values {
            history.push(value);
        }
        Ok(history)
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
