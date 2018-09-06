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
    Branch([Option<K>; 16], Option<V>),
    Leaf(Nibble<T>, V),
    Extension(Nibble<T>, K),
}

impl<T, K, V> Default for Node<T, K, V> {
    fn default() -> Self {
        Node::Empty
    }
}

impl<T, K, V> Node<T, K, V>
where
    T: AsRef<[u8]>,
    // already encoded key
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    pub fn rlp_encoded(&self) -> Vec<u8> {
        match self {
            Node::Leaf(nibble, value) => {
                let mut stream = RlpStream::new_list(2);
                let mut buffer = Vec::new();
                nibble.as_slice().encode(true, &mut buffer);
                stream.append(&buffer);
                stream.append(&value.as_ref());
                stream.drain().into_vec()
            }
            Node::Extension(nibble, key) => {
                let mut stream = RlpStream::new_list(2);
                let mut buffer = Vec::new();
                nibble.as_slice().encode(false, &mut buffer);
                stream.append(&buffer);
                stream.append_raw(&key.as_ref(), 1);
                stream.drain().into_vec()
            }
            Node::Branch(keys, value) => {
                let mut stream = RlpStream::new_list(17);
                for k in keys {
                    match k.as_ref() {
                        Some(k) => {
                            stream.append_raw(&k.as_ref(), 1);
                        }
                        None => {
                            stream.append_empty_data();
                        }
                    }
                }
                match value.as_ref() {
                    Some(k) => {
                        stream.append(&k.as_ref());
                    }
                    None => {
                        stream.append_empty_data();
                    }
                }
                stream.drain().into_vec()
            }
            Node::Empty => {
                let mut stream = RlpStream::new();
                stream.append_empty_data();
                stream.drain().into_vec()
            }
        }
    }
}
