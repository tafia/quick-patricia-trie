use nibbles::Nibble;
use rlp::RlpStream;
use std::mem;

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
    keys: [Option<K>; 16],
    value: Option<V>,
}

impl<K, V> Branch<K, V> {
    pub fn new() -> Self {
        let keys = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];

        Branch { value: None, keys }
    }
    pub fn get(&self, i: u8) -> Option<&K> {
        self.keys.get(i as usize).and_then(|v| v.as_ref())
    }
    pub fn set(&mut self, i: u8, value: Option<K>) {
        self.keys.get_mut(i as usize).map(|v| *v = value);
    }
    pub fn get_value(&self) -> Option<&V> {
        self.value.as_ref()
    }
    pub fn set_value(&mut self, value: Option<V>) {
        self.value = value;
    }
    pub fn take_value(&mut self) -> Option<V> {
        self.value.take()
    }
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
    nibble: Nibble<T>,
    value: V,
}

impl<T, V> Leaf<T, V> {
    pub fn new(nibble: Nibble<T>, value: V) -> Self {
        Leaf { nibble, value }
    }
    pub fn nibble(&self) -> &Nibble<T> {
        &self.nibble
    }
    pub fn set_value(&mut self, value: V) -> V {
        mem::replace(&mut self.value, value)
    }
    pub fn set_nibble(&mut self, nibble: Nibble<T>) -> Nibble<T> {
        mem::replace(&mut self.nibble, nibble)
    }
    pub fn value(self) -> V {
        self.value
    }
    pub fn value_ref(&self) -> &V {
        &self.value
    }
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
    nibble: Nibble<T>,
    key: K,
}

impl<T, K> Extension<T, K> {
    pub fn new(nibble: Nibble<T>, key: K) -> Self {
        Extension { nibble, key }
    }
    pub fn nibble(&self) -> &Nibble<T> {
        &self.nibble
    }
    pub fn set_key(&mut self, key: K) -> K {
        mem::replace(&mut self.key, key)
    }
    pub fn set_nibble(&mut self, nibble: Nibble<T>) -> Nibble<T> {
        mem::replace(&mut self.nibble, nibble)
    }
    pub fn key(self) -> K {
        self.key
    }
    pub fn key_ref(&self) -> &K {
        &self.key
    }
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
