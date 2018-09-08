pub mod merkle;

use node::{Branch, Extension, Leaf, Node};
use std::mem;

/// A trait to handle storage operations
pub trait Storage<T, K, V> {
    fn root() -> K;

    fn get<'a>(&'a self, key: &K) -> Option<&'a Node<T, K, V>>;
    fn get_mut<'a>(&'a mut self, key: &K) -> Option<&'a mut Node<T, K, V>>;
    fn insert_node(&mut self, key: K, node: Node<T, K, V>) -> Option<Node<T, K, V>>;
    fn push_node(&mut self, node: Node<T, K, V>) -> K;
    fn remove(&mut self, key: &K) -> Option<Node<T, K, V>>;

    fn insert_empty(&mut self, key: K) -> Option<Node<T, K, V>> {
        self.insert_node(key, Node::Empty)
    }
    fn insert_leaf(&mut self, key: K, leaf: Leaf<T, V>) -> Option<Node<T, K, V>> {
        self.insert_node(key, Node::Leaf(leaf))
    }
    fn insert_branch(&mut self, key: K, branch: Branch<K, V>) -> Option<Node<T, K, V>> {
        self.insert_node(key, Node::Branch(branch))
    }
    fn insert_extension(&mut self, key: K, extension: Extension<T, K>) -> Option<Node<T, K, V>> {
        self.insert_node(key, Node::Extension(extension))
    }
    fn push_empty(&mut self) -> K {
        self.push_node(Node::Empty)
    }
    fn push_leaf(&mut self, leaf: Leaf<T, V>) -> K {
        self.push_node(Node::Leaf(leaf))
    }
    fn push_branch(&mut self, branch: Branch<K, V>) -> K {
        self.push_node(Node::Branch(branch))
    }
    fn push_extension(&mut self, extension: Extension<T, K>) -> K {
        self.push_node(Node::Extension(extension))
    }
}

pub type VecStorage<T, V> = Vec<Node<T, usize, V>>;

// A basic `Vec<Option<T>>` storage, for testing purpose
impl<T, V> Storage<T, usize, V> for VecStorage<T, V> {
    fn root() -> usize {
        0
    }
    fn get<'a>(&'a self, key: &usize) -> Option<&'a Node<T, usize, V>> {
        (&**self).get(*key)
    }
    fn get_mut<'a>(&'a mut self, key: &usize) -> Option<&'a mut Node<T, usize, V>> {
        (&mut **self).get_mut(*key)
    }
    fn insert_node(&mut self, key: usize, value: Node<T, usize, V>) -> Option<Node<T, usize, V>> {
        if key < self.len() {
            return Some(mem::replace(&mut self[key], value));
        }
        if key > self.len() {
            let len = key - self.len() - 1;
            self.extend((0..len).map(|_| Node::Empty));
        }
        self.push(value);
        None
    }
    fn insert_extension(
        &mut self,
        key: usize,
        extension: Extension<T, usize>,
    ) -> Option<Node<T, usize, V>> {
        self.insert_node(key, Node::Extension(extension))
    }
    fn push_node(&mut self, node: Node<T, usize, V>) -> usize {
        self.push(node);
        self.len() - 1
    }
    fn remove(&mut self, key: &usize) -> Option<Node<T, usize, V>> {
        self.insert_node(*key, Node::Empty)
    }
}
