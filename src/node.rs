use arena::Arena;
use db::Index;
use keccak_hash::{keccak, H256};
use nibbles::Nibble;
use rlp::RlpStream;

/// A trie `Node`
#[derive(Debug)]
pub enum Node {
    Empty,
    Branch(Branch),
    Leaf(Leaf),
    Extension(Extension),
}

#[derive(Debug)]
pub struct Branch {
    pub keys: [Option<Index>; 16],
    pub value: Option<usize>,
}

impl Branch {
    pub fn new() -> Self {
        let keys = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        Branch { keys, value: None }
    }

    /// RLP encode the branch
    ///
    /// Returns None if any key is not hashed
    pub fn try_hash(&self, arena: &mut Arena, indexes: &[Option<usize>]) -> Option<usize> {
        let mut keys = Vec::with_capacity(16);
        for k in self.keys.iter() {
            match k {
                Some(Index::Memory(ref i)) => match indexes[*i] {
                    Some(k) => keys.push(Some(k)),
                    None => return None,
                },
                Some(Index::Hash(ref k)) => keys.push(Some(*k)),
                None => keys.push(None),
            }
        }
        let mut stream = RlpStream::new_list(17);
        for k in keys.into_iter() {
            match k {
                None => {
                    stream.append_empty_data();
                }
                Some(i) => {
                    stream.append_raw(&arena.get(i), 1);
                }
            }
        }
        match self.value.as_ref() {
            None => {
                stream.append_empty_data();
            }
            Some(i) => {
                stream.append(&arena.get(*i));
            }
        }
        Some(hash_or_inline(&stream.drain(), arena))
    }
}

#[derive(Debug, Clone)]
pub struct Leaf {
    pub nibble: Nibble,
    pub value: usize,
}

impl Leaf {
    pub fn new<N: AsRef<[u8]>, V: AsRef<[u8]>>(nibble: N, value: V, arena: &mut Arena) -> Leaf {
        let nibble = Nibble::new(nibble, arena);
        let value = arena.push(value.as_ref());
        Leaf { nibble, value }
    }

    /// RLP encode the leaf
    ///
    /// Always work
    pub fn try_hash(&self, arena: &mut Arena) -> usize {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(true, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append(&arena.get(self.value));
        hash_or_inline(&stream.drain(), arena)
    }
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub nibble: Nibble,
    pub key: Index,
}

impl Extension {
    /// RLP encode the extension
    ///
    /// Returns None if the key is not hashed
    pub fn try_hash(&self, arena: &mut Arena, indexes: &[Option<usize>]) -> Option<usize> {
        let key = match self.key {
            Index::Hash(ref key) => *key,
            Index::Memory(ref i) => {
                if let Some(k) = indexes[*i] {
                    k
                } else {
                    return None;
                }
            }
        };
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(false, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append_raw(&arena.get(key), 1);
        Some(hash_or_inline(&stream.drain(), arena))
    }
}

impl Node {
    pub fn try_hash(&self, arena: &mut Arena, indexes: &[Option<usize>]) -> Option<usize> {
        match self {
            Node::Leaf(leaf) => Some(leaf.try_hash(arena)),
            Node::Extension(extension) => extension.try_hash(arena, indexes),
            Node::Branch(branch) => branch.try_hash(arena, indexes),
            Node::Empty => {
                let mut stream = RlpStream::new();
                stream.append_empty_data();
                Some(hash_or_inline(&stream.drain(), arena))
            }
        }
    }
}

#[inline]
fn hash_or_inline(data: &[u8], arena: &mut Arena) -> usize {
    if data.len() <= H256::len() {
        arena.push(data)
    } else {
        arena.push(keccak(data).as_ref())
    }
}
