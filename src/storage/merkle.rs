use keccak_hash::{keccak, H256};
use node::Node;
use std::collections::HashMap;
use storage::Storage;
use plain_hasher::PlainHasher;
use std::hash;

/// A Merkle Storage where key = sha3(rlp(value))
pub struct MerkleStorage<T, V> {
    root: H256,
    db: HashMap<H256, Node<T, H256, V>, hash::BuildHasherDefault<PlainHasher>>,
}

impl<T, V> MerkleStorage<T, V> {
    pub fn new() -> Self {
        MerkleStorage {
            root: keccak(::rlp::NULL_RLP),
            db: HashMap::default(),
        }
    }
}

impl<T, V> Storage for MerkleStorage<T, V>
where
    T: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    type Key = H256;
    type Value = Node<T, H256, V>;

    fn root(&self) -> H256 {
        self.root.clone()
    }

    fn get<'a>(&'a self, key: &Self::Key) -> Option<&'a Self::Value> {
        self.db.get(key)
    }

    fn get_mut<'a>(&'a mut self, key: &Self::Key) -> Option<&'a mut Self::Value> {
        self.db.get_mut(key)
    }
    fn insert(&mut self, key: Self::Key, value: Self::Value) -> Option<Self::Value> {
        self.db.insert(key, value)
    }
    fn push(&mut self, value: Self::Value) -> Self::Key {
        let encoded_value = value.rlp_encoded();
        let key = keccak(encoded_value);
        self.db.insert(key.clone(), value);
        key
    }
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value> {
        self.db.insert(key.clone(), Node::Empty)
    }
}
