use keccak_hash::{keccak, KECCAK_NULL_RLP};
use node::{Branch, Extension, Leaf, Node};
//use plain_hasher::PlainHasher;
use std::collections::HashMap;
//use std::hash;
use storage::Arena;

/// A Merkle Storage where key = sha3(rlp(value))
#[derive(Debug)]
pub struct MerkleStorage {
    db: HashMap<usize, Node>,
    // db: HashMap<usize, Node, hash::BuildHasherDefault<PlainHasher>>,
    root: usize,
}

impl MerkleStorage {
    pub fn new(arena: &mut Arena) -> Self {
        let idx = arena.push(KECCAK_NULL_RLP.as_ref());
        MerkleStorage {
            db: HashMap::default(),
            root: idx,
        }
    }

    pub fn root(&self) -> usize {
        self.root
    }

    pub fn get<'a>(&'a self, key: &usize) -> Option<&'a Node> {
        self.db.get(key)
    }
    pub fn get_mut<'a>(&'a mut self, key: &usize) -> Option<&'a mut Node> {
        self.db.get_mut(key)
    }

    #[inline]
    pub fn insert_node(&mut self, key: usize, value: Node) -> Option<Node> {
        debug!("inserting node {}", key);
        self.db.insert(key, value)
    }

    pub fn push_leaf(&mut self, leaf: Leaf, arena: &mut Arena) -> usize {
        let encoded_value = leaf.rlp_encoded(arena);
        let key = keccak(encoded_value);
        let idx_key = arena.push(key.as_ref());
        debug!("pushing leaf {}", idx_key);
        self.db.insert(idx_key, Node::Leaf(leaf));
        idx_key
    }
    pub fn push_branch(&mut self, branch: Branch, arena: &mut Arena) -> usize {
        let encoded_value = branch.rlp_encoded(arena);
        let key = keccak(encoded_value);
        let idx_key = arena.push(key.as_ref());
        debug!("pushing branch {}", idx_key);
        self.db.insert(idx_key, Node::Branch(branch));
        idx_key
    }
    pub fn push_extension(&mut self, extension: Extension, arena: &mut Arena) -> usize {
        let encoded_value = extension.rlp_encoded(arena);
        let key = keccak(encoded_value);
        let idx_key = arena.push(key.as_ref());
        debug!("pushing extension {}", idx_key);
        self.db.insert(idx_key, Node::Extension(extension));
        idx_key
    }
    #[inline]
    pub fn remove(&mut self, key: &usize) -> Option<Node> {
        debug!("removing node {}", key);
        self.db.insert(*key, Node::Empty)
    }
}
