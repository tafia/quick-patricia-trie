use keccak_hash::{keccak, KECCAK_NULL_RLP};
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
        MerkleStorage {
            hash: HashMap::new(),
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

    pub fn get_mut<'a>(&'a mut self, key: &Index) -> Option<&'a mut Node> {
        match key {
            Index::Hash(ref key) => self.hash.get_mut(key),
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
    pub fn commit(&mut self, arena: &mut Arena) {
        // create a queue of nodes to commit
        let mut queue = self.memory.drain(..).enumerate().collect::<VecDeque<_>>();
        while let Some((i, node)) = queue.pop_back() {
            match node.rlp_encoded(arena) {
                None => queue.push_front((i, node)),
                Some(encoded_value) => {
                    let key = keccak(encoded_value);
                    let idx_hash = arena.push(key.as_ref());
                    self.hash.insert(idx_hash, node);

                    // update all the queue with the new index
                    for &mut (_, ref mut node) in queue.iter_mut() {
                        match node {
                            Node::Extension(ref mut ext) if ext.key == Index::Memory(i) => {
                                ext.key = Index::Hash(idx_hash);
                            }
                            Node::Branch(ref mut branch) => {
                                for k in branch.keys.iter_mut() {
                                    if let Some(ref mut k) = k {
                                        if *k == Index::Memory(i) {
                                            *k = Index::Hash(idx_hash);
                                        }
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                }
            }
        }
    }
}
