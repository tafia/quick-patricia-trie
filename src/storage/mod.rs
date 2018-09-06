pub mod merkel;

use std::mem;

/// A trait to handle storage operations
pub trait Storage {
    /// Storage key
    type Key;
    /// Storage value
    type Value;
    fn root(&self) -> Self::Key;
    /// Get item at Key
    fn get<'a>(&'a self, key: &Self::Key) -> Option<&'a Self::Value>;
    /// Get mutable item at Key
    fn get_mut<'a>(&'a mut self, key: &Self::Key) -> Option<&'a mut Self::Value>;
    /// Insert item at key
    fn insert(&mut self, key: Self::Key, value: Self::Value) -> Option<Self::Value>;
    /// Push a new item and returns the new key
    fn push(&mut self, value: Self::Value) -> Self::Key;
    /// Remove item at key
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value>;
}

// A basic `Vec<Option<T>>` storage, for testing purpose
impl<T> Storage for Vec<Option<T>> {
    type Key = usize;
    type Value = T;
    fn root(&self) -> Self::Key {
        0
    }
    fn get<'a>(&'a self, key: &Self::Key) -> Option<&'a Self::Value> {
        self.as_slice().get(*key).and_then(|v| v.as_ref())
    }
    fn get_mut<'a>(&'a mut self, key: &Self::Key) -> Option<&'a mut Self::Value> {
        self.as_mut_slice().get_mut(*key).and_then(|v| v.as_mut())
    }
    fn insert(&mut self, key: Self::Key, value: Self::Value) -> Option<Self::Value> {
        if key < self.len() {
            return mem::replace(&mut self[key], Some(value));
        }
        if key > self.len() {
            let len = key - self.len() - 1;
            self.extend((0..len).map(|_| None));
        }
        self.push(Some(value));
        None
    }
    fn push(&mut self, value: Self::Value) -> Self::Key {
        self.push(Some(value));
        self.len() - 1
    }
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value> {
        mem::replace(&mut self[*key], None)
    }
}
