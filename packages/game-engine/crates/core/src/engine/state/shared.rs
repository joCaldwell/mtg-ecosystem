//! Copy-on-write ownership of one state table.
//!
//! A staged fork shares the table. The first exclusive access copies it for
//! that fork alone, so a rule step pays only for the tables it writes. The
//! wire form and equality are the table's own; sharing is never observable.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Default)]
pub struct Shared<T>(Arc<T>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }
    /// True when both handles are the same allocation: the tables are equal
    /// and neither has been written since they were shared.
    pub fn same(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}
impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: Clone> DerefMut for Shared<T> {
    /// Exclusive access. Copies the table first if a fork still shares it.
    fn deref_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}
impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        // `Eq` tables are reflexive, so a shared allocation proves equality.
        Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}
impl<T: Eq> Eq for Shared<T> {}
impl<T: PartialEq> PartialEq<T> for Shared<T> {
    fn eq(&self, other: &T) -> bool {
        *self.0 == *other
    }
}
impl<T: fmt::Debug> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::new)
    }
}
impl<'a, T> IntoIterator for &'a Shared<T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;
    type IntoIter = <&'a T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.as_ref().into_iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut Shared<T>
where
    &'a mut T: IntoIterator,
{
    type Item = <&'a mut T as IntoIterator>::Item;
    type IntoIter = <&'a mut T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        Arc::make_mut(&mut self.0).into_iter()
    }
}
impl<A, T: FromIterator<A>> FromIterator<A> for Shared<T> {
    fn from_iter<I: IntoIterator<Item = A>>(items: I) -> Self {
        Self::new(T::from_iter(items))
    }
}
