use arena::Arena;
use db::Index;
use keccak_hash::H256;
use nibbles::Nibble;
use rlp::{DecoderError, Prototype, Rlp, RlpStream};

/// A trie `Node`
#[derive(Debug)]
pub enum Node {
    Empty,
    Branch(Branch),
    Leaf(Leaf),
    Extension(Extension),
}

impl Node {
    pub fn try_from_encoded(data: &[u8], arena: &mut Arena) -> Option<Self> {
        match Node::from_encoded_res(&data, arena) {
            Ok(n) => Some(n),
            Err(e) => {
                error!("Error decoding rlp node {}", e);
                None
            }
        }
    }

    fn from_encoded_res(data: &[u8], arena: &mut Arena) -> Result<Self, DecoderError> {
        let r = Rlp::new(data);
        match r.prototype()? {
            Prototype::List(2) => {
                let nibble = arena.push(r.at(0)?.data()?);
                let value = arena.push(r.at(1)?.data()?);
                match Nibble::from_encoded(nibble, arena) {
                    (true, nibble) => Ok(Node::Leaf(Leaf { nibble, value })),
                    (false, nibble) => Ok(Node::Extension(Extension {
                        nibble,
                        key: Index::Hash(value),
                    })),
                }
            }
            Prototype::List(17) => {
                let mut branch = Branch::default();
                for i in 0..16 {
                    let key = r.at(i)?.as_raw();
                    if !key.is_empty() {
                        branch.keys[i] = Some(Index::Hash(arena.push(key)));
                    }
                }
                let value = r.at(16)?;
                if !value.is_empty() {
                    branch.value = Some(arena.push(value.data()?));
                }
                Ok(Node::Branch(branch))
            }
            Prototype::Data(0) => Ok(Node::Empty),
            _ => Err(DecoderError::Custom("Rlp is not valid.")),
        }
    }
}

#[derive(Debug, Default)]
pub struct Branch {
    pub keys: [Option<Index>; 16],
    pub value: Option<usize>,
}

impl Branch {
    /// RLP encode the branch
    ///
    /// Ignores Memory nodes
    pub fn encoded(&mut self, arena: &mut Arena) -> usize {
        let mut stream = RlpStream::new_list(17);
        for k in &self.keys {
            match k {
                Some(Index::Hash(i)) => {
                    let data = &arena[*i];
                    if data.len() < H256::len() {
                        // inlined
                        stream.append_raw(&data, 1);
                    } else {
                        stream.append(&data);
                    }
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
                stream.append(&&arena[*i]);
            }
        }
        arena.push(&stream.drain())
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
    pub fn encoded(&self, arena: &mut Arena) -> usize {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(true, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append(&&arena[self.value]);
        arena.push(&stream.drain())
    }
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub nibble: Nibble,
    pub key: Index,
}

impl Extension {
    /// RLP encode the extension
    pub fn encoded_or_empty(&mut self, arena: &mut Arena, empty: usize) -> usize {
        let key = if let Index::Hash(i) = self.key {
            i
        } else {
            warn!("hashing memory extension");
            return empty;
        };

        let mut stream = RlpStream::new_list(2);
        stream.append(&self.nibble.encoded(false, arena));

        {
            let key = &arena[key];
            if key.len() < H256::len() {
                // inline already encoded data
                stream.append_raw(key, 1);
            } else {
                stream.append(&key);
            }
        }
        arena.push(&stream.drain())
    }
}
