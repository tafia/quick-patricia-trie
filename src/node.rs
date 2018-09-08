use nibbles::Nibble;
use rlp::RlpStream;

/// A trie `Node`
///
/// - `T` is the nibble inner container
/// - `K` is the database key
/// - `V` is the database value
#[derive(Debug)]
pub enum Node<T, K, V> {
    Empty,
    Branch(Branch<K, V>),
    Leaf(Leaf<T, V>),
    Extension(Extension<T, K>),
}

impl<T, K, V> Default for Node<T, K, V> {
    fn default() -> Self {
        Node::Empty
    }
}

#[derive(Debug)]
pub struct Branch<K, V> {
    pub keys: [Option<K>; 16],
    pub value: Option<V>,
}

impl<K: AsRef<[u8]>, V: AsRef<[u8]>> Branch<K, V> {
    pub fn rlp_encoded(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(17);
        for k in self.keys.iter() {
            if let Some(k) = k {
                stream.append_raw(&k.as_ref(), 1);
            } else {
                stream.append_empty_data();
            }
        }
        if let Some(ref k) = self.value {
            stream.append(&k.as_ref());
        } else {
            stream.append_empty_data();
        }
        stream.out()
    }
}

#[derive(Debug)]
pub struct Leaf<T, V> {
    pub nibble: Nibble<T>,
    pub value: V,
}

impl<T: AsRef<[u8]>, V: AsRef<[u8]>> Leaf<T, V> {
    pub fn rlp_encoded(&self) -> Vec<u8> {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.as_slice().encoded(true);
        stream
            .begin_list(2)
            .append(&buffer)
            .append(&self.value.as_ref());
        stream.out()
    }
}

#[derive(Debug)]
pub struct Extension<T, K> {
    pub nibble: Nibble<T>,
    pub key: K,
}

impl<T: AsRef<[u8]>, K: AsRef<[u8]>> Extension<T, K> {
    pub fn rlp_encoded(&self) -> Vec<u8> {
        let mut stream = RlpStream::new();
        let buffer = self.nibble.as_slice().encoded(false);
        stream
            .begin_list(2)
            .append(&buffer)
            .append_raw(&self.key.as_ref(), 1);
        stream.out()
    }
}

impl<T, K, V> Node<T, K, V>
where
    T: AsRef<[u8]>,
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    pub fn rlp_encoded(&self) -> Vec<u8> {
        match self {
            Node::Leaf(leaf) => leaf.rlp_encoded(),
            Node::Extension(extension) => extension.rlp_encoded(),
            Node::Branch(branch) => branch.rlp_encoded(),
            Node::Empty => {
                let mut stream = RlpStream::new();
                stream.append_empty_data();
                stream.out()
            }
        }
    }
}
