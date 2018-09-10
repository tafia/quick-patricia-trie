use keccak_hash::KECCAK_NULL_RLP;
use node::{Branch, Extension, Leaf, Node};
use std::collections::{HashMap, VecDeque};
use std::mem;
use storage::Arena;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Index {
    Hash(usize),
    Memory(usize),
}

impl Default for Index {
    fn default() -> Self {
        Index::Hash(0)
    }
}

/// A Merkle Storage
///
/// Nodes are either stored in a simple Vec memory
/// or pushed into a *database* with key = sha3(rlp(value))
#[derive(Debug)]
pub struct MerkleStorage {
    hash: HashMap<usize, Node>,
    memory: Vec<Node>,
    root: Index,
}

impl MerkleStorage {
    pub fn new(arena: &mut Arena) -> Self {
        let idx = arena.push(KECCAK_NULL_RLP.as_ref());
        let mut hash = HashMap::new();
        hash.insert(idx, Node::Empty);
        MerkleStorage {
            hash,
            memory: Vec::new(),
            root: Index::Hash(idx),
        }
    }

    pub fn root(&self) -> Index {
        self.root
    }

    pub fn get<'a>(&'a self, key: &Index) -> Option<&'a Node> {
        match key {
            Index::Hash(ref key) => self.hash.get(key),
            Index::Memory(ref key) => self.memory.get(*key),
        }
    }

    /// Get a mutable reference to node at key
    ///
    /// If the key is hashed, then moves the node into memory first
    pub fn get_mut<'a>(&'a mut self, key: &mut Index) -> Option<&'a mut Node> {
        match key.clone() {
            Index::Hash(hash) => {
                let node = self.hash.remove(&hash)?;
                let len = self.memory.len();
                if *key == self.root {
                    self.root = Index::Memory(len);
                }
                debug!("hash {} moved to memory {}", hash, len);
                *key = Index::Memory(len);
                self.memory.push(node);
                self.memory.get_mut(len)
            }
            Index::Memory(ref key) => self.memory.get_mut(*key),
        }
    }

    pub fn insert_node(&mut self, key: Index, value: Node) -> Option<Node> {
        debug!("inserting node {:?}", key);
        match key {
            Index::Hash(key) => self.hash.insert(key, value),
            Index::Memory(key) => self.memory.get_mut(key).map(|v| mem::replace(v, value)),
        }
    }

    pub fn push_leaf(&mut self, leaf: Leaf) -> Index {
        let index = Index::Memory(self.memory.len());
        debug!("pushing leaf {:?}", index);
        self.memory.push(Node::Leaf(leaf));
        index
    }

    pub fn push_branch(&mut self, branch: Branch) -> Index {
        let index = Index::Memory(self.memory.len());
        debug!("pushing branch {:?}", index);
        self.memory.push(Node::Branch(branch));
        index
    }

    pub fn push_extension(&mut self, extension: Extension) -> Index {
        let index = Index::Memory(self.memory.len());
        debug!("pushing extension {:?}", index);
        self.memory.push(Node::Extension(extension));
        index
    }

    pub fn remove(&mut self, key: &Index) -> Option<Node> {
        debug!("removing node {:?}", key);
        match key {
            Index::Hash(key) => self.hash.insert(*key, Node::Empty),
            Index::Memory(key) => self
                .memory
                .get_mut(*key)
                .map(|v| mem::replace(v, Node::Empty)),
        }
    }

    /// Commit all the in memory nodes into hash db
    pub fn commit<'a>(&mut self, arena: &'a mut Arena) -> Option<&'a [u8]> {
        // create a queue of nodes to commit
        let mut indexes = vec![None; self.memory.len()];
        let mut queue = self.memory.drain(..).enumerate().collect::<VecDeque<_>>();
        while let Some((i, node)) = queue.pop_back() {
            match node.build_hash(arena, &indexes) {
                None => queue.push_front((i, node)),
                Some(idx_hash) => {
                    self.hash.insert(idx_hash, node);
                    indexes[i] = Some(idx_hash);
                }
            }
        }

        match self.root {
            Index::Memory(i) => {
                if let Some(i) = indexes[i] {
                    self.root = Index::Hash(i);
                    Some(arena.get(i))
                } else {
                    None
                }
            }
            Index::Hash(i) => Some(arena.get(i)),
        }
    }
}
