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
    /// Ignores Memory nodes
    pub fn hash(&mut self, arena: &mut Arena) -> usize {
        let mut stream = RlpStream::new_list(17);
        for k in self.keys.iter() {
            match k {
                Some(Index::Hash(i)) => {
                    stream.append_raw(&arena.get(*i), 1);
                }
                _ => {
                    stream.append_empty_data();
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
        hash_or_inline(&stream.drain(), arena)
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
    pub fn hash(&self, arena: &mut Arena) -> usize {
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
    pub fn hash_or_empty(&mut self, arena: &mut Arena, empty: usize) -> usize {
        let key = if let Index::Hash(i) = self.key {
            i
        } else {
            return empty;
        };
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(false, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append_raw(&arena.get(key), 1);
        hash_or_inline(&stream.drain(), arena)
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
