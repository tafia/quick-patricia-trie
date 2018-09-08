use keccak_hash::{keccak, H256, KECCAK_NULL_RLP};
use node::{Branch, Extension, Leaf, Node};
use plain_hasher::PlainHasher;
use std::collections::HashMap;
use std::hash;
use storage::Storage;

/// A Merkle Storage where key = sha3(rlp(value))
#[derive(Debug)]
pub struct MerkleStorage<T, V> {
    db: HashMap<H256, Node<T, H256, V>, hash::BuildHasherDefault<PlainHasher>>,
}

impl<T, V> MerkleStorage<T, V> {
    pub fn new() -> Self {
        MerkleStorage {
            db: HashMap::default(),
        }
    }
}

impl<T, V> Storage<T, H256, V> for MerkleStorage<T, V>
where
    T: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    fn root() -> H256 {
        KECCAK_NULL_RLP
    }

    fn get<'a>(&'a self, key: &H256) -> Option<&'a Node<T, H256, V>> {
        self.db.get(key)
    }
    fn get_mut<'a>(&'a mut self, key: &H256) -> Option<&'a mut Node<T, H256, V>> {
        self.db.get_mut(key)
    }

    #[inline]
    fn insert_node(&mut self, key: H256, value: Node<T, H256, V>) -> Option<Node<T, H256, V>> {
        self.db.insert(key, value)
    }

    #[inline]
    fn push_node(&mut self, value: Node<T, H256, V>) -> H256 {
        let encoded_value = value.rlp_encoded();
        let key = keccak(encoded_value);
        self.db.insert(key.clone(), value);
        key
    }
    fn push_empty(&mut self) -> H256 {
        // do nothing and return root
        KECCAK_NULL_RLP
    }
    fn push_leaf(&mut self, leaf: Leaf<T, V>) -> H256 {
        let encoded_value = leaf.rlp_encoded();
        let key = keccak(encoded_value);
        self.db.insert(key.clone(), Node::Leaf(leaf));
        key
    }
    fn push_branch(&mut self, branch: Branch<H256, V>) -> H256 {
        let encoded_value = branch.rlp_encoded();
        let key = keccak(encoded_value);
        self.db.insert(key.clone(), Node::Branch(branch));
        key
    }
    fn push_extension(&mut self, extension: Extension<T, H256>) -> H256 {
        let encoded_value = extension.rlp_encoded();
        let key = keccak(encoded_value);
        self.db.insert(key.clone(), Node::Extension(extension));
        key
    }
    #[inline]
    fn remove(&mut self, key: &H256) -> Option<Node<T, H256, V>> {
        self.insert_empty(*key)
    }
}
