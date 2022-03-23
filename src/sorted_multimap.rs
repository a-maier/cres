use std::cmp::{PartialEq, PartialOrd};
use std::clone::Clone;
use std::collections::{BinaryHeap, TryReserveError};

use delegate::delegate;
use derive_more::{From, Into, IntoIterator, AsRef, Index, Deref};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, IntoIterator, From, Into, AsRef, Index, Deref)]
pub struct SortedMultimap<K, V> {
    e: Vec<(K, V)>,
}

impl<K: PartialOrd, V> std::default::Default for SortedMultimap<K, V> {
    fn default() -> Self {
        Self{ e: Vec::new() }
    }
}

impl<K: PartialOrd, V> SortedMultimap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(c: usize) -> Self {
        Self{ e: Vec::with_capacity(c) }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let idx = self.e
            .binary_search_by(|(k, _)| k.partial_cmp(&key).unwrap())
            .unwrap_or_else(|x| x);
        self.e.insert(idx, (key, value))
    }

    pub fn get_all(&mut self, key: &K) -> &[(K, V)]
    where K: PartialEq
    {
        let idx = self.e
            .binary_search_by(|(k, _)| k.partial_cmp(&key).unwrap());
        if let Ok(idx) = idx {
            let len = self.e.len();
            let begin = self.e.iter().rev().skip(len - idx - 1).position(
                |a| key != &a.0
            ).map(|pos| idx - pos + 1).unwrap_or(0);
            let end = self.e.iter().skip(idx + 1).position(
                |a| key != &a.0
            ).map(|pos| idx + 1 + pos).unwrap_or(len);
            &self.e[begin..end]
        } else {
            &[]
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&(K, V)>
    where K: PartialEq
    {
        let idx = self.e
            .binary_search_by(|(k, _)| k.partial_cmp(&key).unwrap());
        if let Ok(idx) = idx {
            Some(&self.e[idx])
        } else {
            None
        }
    }

    delegate!{ to self.e {
        pub fn as_ptr(&self) -> *const (K, V);
        pub fn as_slice(&self) -> &[(K, V)];
        pub fn capacity(&self) -> usize;
        pub fn clear(&mut self);
        pub fn is_empty(&self) -> bool;
        pub fn len(&self) -> usize;
        #[call(pop)]
        pub fn pop_largest(&mut self) ->Option<(K, V)>;
        #[call(remove)]
        pub fn remove_at(&mut self, index: usize) -> (K, V);
        pub fn reserve(&mut self, additional: usize);
        pub fn reserve_exact(&mut self, additional: usize);
        pub fn retain<F: FnMut(&(K, V)) -> bool>(&mut self, f: F);
        pub unsafe fn set_len(&mut self, new_len: usize);
        pub fn shrink_to(&mut self, min_capacity: usize);
        pub fn shrink_to_fit(&mut self);
        pub fn truncate(&mut self, len: usize);
        pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError>;
        pub fn try_reserve_exact(
            &mut self,
            additional: usize
        ) -> Result<(), TryReserveError>;
    }}
}

impl<K: PartialOrd, V> FromIterator<(K, V)> for SortedMultimap<K, V> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>
    {
        let mut e = Vec::from_iter(iter);
        e.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { e }
    }
}

impl<K: PartialOrd + Clone, V: Clone> From<&[(K, V)]> for SortedMultimap<K, V> {
    fn from(f: &[(K, V)]) -> Self {
        let mut e = Vec::from(f);
        e.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { e }
    }
}

impl<K: PartialOrd + Clone, V: Clone> From<&mut [(K, V)]> for SortedMultimap<K, V> {
    fn from(f: &mut [(K, V)]) -> Self {
        let mut e = Vec::from(f);
        e.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { e }
    }
}

impl<K: PartialOrd, V, const N: usize> From<[(K, V); N]> for SortedMultimap<K, V> {
    fn from(f: [(K, V); N]) -> Self {
        let mut e = Vec::from(f);
        e.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { e }
    }
}

impl<K: PartialOrd, V> From<BinaryHeap<(K, V)>> for SortedMultimap<K, V> {
    fn from(f: BinaryHeap<(K, V)>) -> Self {
        let mut e = Vec::from(f);
        e.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self { e }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct() {
        let mut map = SortedMultimap::new();
        map.insert(2, 3);
        map.insert(0, 1);
        map.insert(1, 2);

        assert_eq!(
            map,
            SortedMultimap::from([(0, 1), (1, 2), (2, 3)])
        )
    }

    #[test]
    fn get() {
        let mut map = SortedMultimap::from([(0, 1), (0, 2), (0, 3)]);

        assert_eq!(map.get_all(&0).len(), map.len());

        map.insert(-1, 0);
        map.insert(3, 0);
        assert!(map.get(&0).is_some());
        assert!(map.get(&3).is_some());
        assert_eq!(map.get(&-5), None);
        assert_eq!(map.get_all(&0).len(), map.len() - 2);
        assert_eq!(map.get_all(&3).len(), 1);
        assert!(map.get_all(&-5).is_empty());
    }
}
