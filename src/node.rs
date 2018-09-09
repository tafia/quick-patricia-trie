use nibbles::Nibble;
use rlp::RlpStream;
use storage::Arena;

/// A trie `Node`
///
/// - `T` is the nibble inner container
/// - `K` is the database key
/// - `V` is the database value
#[derive(Debug)]
pub enum Node {
    Empty,
    Branch(Branch),
    Leaf(Leaf),
    Extension(Extension),
}

impl Default for Node {
    fn default() -> Self {
        Node::Empty
    }
}

#[derive(Debug)]
pub struct Branch {
    pub keys: [Option<usize>; 16],
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
    pub fn rlp_encoded(&self, arena: &Arena) -> Vec<u8> {
        let mut stream = RlpStream::new_list(17);
        for k in self.keys.iter().cloned() {
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
        stream.out()
    }
}

#[derive(Debug, Default, Clone)]
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

    pub fn rlp_encoded(&self, arena: &Arena) -> Vec<u8> {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(true, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append(&arena.get(self.value));
        stream.out()
    }
}

#[derive(Debug, Default, Clone)]
pub struct Extension {
    pub nibble: Nibble,
    pub key: usize,
}

impl Extension {
    pub fn rlp_encoded(&self, arena: &Arena) -> Vec<u8> {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.encoded(false, arena);
        stream
            .begin_list(2)
            .append(&buffer)
            .append_raw(&arena.get(self.key), 1);
        stream.out()
    }
}

impl Node {
    pub fn rlp_encoded(&self, arena: &Arena) -> Vec<u8> {
        match self {
            Node::Leaf(leaf) => leaf.rlp_encoded(arena),
            Node::Extension(extension) => extension.rlp_encoded(arena),
            Node::Branch(branch) => branch.rlp_encoded(arena),
            Node::Empty => {
                let mut stream = RlpStream::new();
                stream.append_empty_data();
                stream.out()
            }
        }
    }
}
