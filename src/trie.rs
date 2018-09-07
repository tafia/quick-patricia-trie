use nibbles::Nibble;
use node::Node;
use std::mem;
use storage::Storage;

/// A patricia trie
///
/// - `S` is the storage type
/// - `T` is the nibble inner container
/// - `K` is the database key
/// - `V` is the database value
///
///   => S is generally like a `Container<K, Node<T, K, V>>`
#[derive(Debug)]
pub struct Trie<S> {
    db: S,
}

impl<S> Trie<S> {
    pub fn new(db: S) -> Self {
        Trie { db }
    }

    pub fn db(&self) -> &S {
        &self.db
    }

    pub fn into_db(self) -> S {
        self.db
    }
}

impl<S, T, K, V> Trie<S>
where
    S: Storage<Key = K, Value = Node<T, K, V>>,
    T: AsRef<[u8]> + ::std::fmt::Debug,
    K: Clone + PartialEq,
    V: ::std::fmt::Debug,
{
    /// Get the item corresponding to that nibble
    pub fn get<'a, Q>(&'a self, path: Nibble<Q>) -> Option<&'a V>
    where
        T: 'a,
        K: 'a,
        Q: AsRef<[u8]>,
    {
        let mut key = &self.db.root();
        let mut path = path.as_slice();
        loop {
            match self.db.get(key)? {
                Node::Branch(arr, v) => {
                    if let Some((u, n)) = path.split_first() {
                        key = arr.get(u as usize)?.as_ref()?;
                        path = n;
                    } else {
                        return v.as_ref();
                    }
                }
                Node::Extension(n, v) => {
                    path = path.split_start(&n.as_slice())?;
                    key = v;
                }
                Node::Leaf(n, v) => {
                    return if *n == path { Some(v) } else { None };
                }
                Node::Empty => return None,
            }
        }
    }

    /// Remove the item corresponding to that nibble
    pub fn remove<Q>(&mut self, path: Nibble<Q>) -> Option<V>
    where
        Q: AsRef<[u8]>,
    {
        // In practice we have several scenarii depending on the final node:
        // - if node = Leaf && use_empty_node = true => set node as Empty
        // - if node = Leaf && use_empty_node = false => remove node from db
        // - if node = Branch && value is Some => set value to None
        // - if node = Branch && value is None => do nothing
        let (is_branch, key) = {
            let mut key = &self.db.root();
            let mut path = path.as_slice();
            let is_branch = loop {
                match self.db.get(key)? {
                    Node::Branch(arr, _v) => {
                        if let Some((u, n)) = path.split_first() {
                            key = arr.get(u as usize)?.as_ref()?;
                            path = n;
                        } else {
                            break true;
                        }
                    }
                    Node::Extension(n, v) => {
                        path = path.split_start(&n.as_slice())?;
                        key = v;
                    }
                    Node::Leaf(n, _v) if *n == path => break false,
                    _ => return None,
                }
            };
            (is_branch, key.clone())
        };
        if is_branch {
            match self.db.get_mut(&key)? {
                Node::Branch(_, v) => v.take(),
                _ => None,
            }
        } else {
            match self.db.remove(&key)? {
                Node::Leaf(_, v) => Some(v),
                _ => None,
            }
        }
    }

    /// Insert the (path, value), return the existing value for that path, if any
    pub fn insert(&mut self, path: Nibble<T>, value: V) -> Option<V>
    where
        Nibble<T>: From<Nibble<Vec<u8>>>,
    {
        debug!("inserting ({:?}, {:?})", path, value);
        enum Action {
            InsertLeaf, // if key not found of Node = Empty
            BranchValue,
            BranchKey(u8),
            BreakLeaf,
            BreakExtension,
        }

        // determine which action needs to be done (pure borrow)
        let (key, path, action) = {
            let mut path = path.as_slice();
            let mut key = &self.db.root();
            let action = loop {
                match self.db.get(key) {
                    Some(Node::Branch(arr, _)) => {
                        if let Some((u, n)) = path.split_first() {
                            path = n;
                            match arr.get(u as usize)?.as_ref() {
                                Some(k) => key = k,
                                None => break Action::BranchKey(u),
                            }
                        } else {
                            break Action::BranchValue;
                        }
                    }
                    Some(Node::Extension(n, v)) => match path.split_start(&n.as_slice()) {
                        Some(p) => {
                            path = p;
                            key = v;
                        }
                        None => break Action::BreakExtension,
                    },
                    Some(Node::Leaf(_, _)) => break Action::BreakLeaf,
                    Some(Node::Empty) | None => break Action::InsertLeaf,
                }
            };
            (key.clone(), path.to_vec(), action)
        };

        // insert the value with eventually intermediary nodes
        match action {
            Action::InsertLeaf => match self.db.insert(key, Node::Leaf(path.into(), value)) {
                Some(Node::Leaf(_, v)) => Some(v),
                _ => None,
            },
            Action::BranchValue => match self.db.get_mut(&key) {
                Some(Node::Branch(_, ref mut v)) => mem::replace(v, Some(value)),
                _ => None,
            },
            Action::BranchKey(idx) => {
                let new_key = self.db.push(Node::Leaf(path.into(), value));
                match self.db.get_mut(&key) {
                    Some(Node::Branch(ref mut keys, _)) => keys[idx as usize] = Some(new_key),
                    _ => (),
                }
                None
            }
            Action::BreakLeaf => self.break_leaf(key, path, value),
            Action::BreakExtension => self.break_extension(key, path, value),
        }
    }

    fn break_leaf(&mut self, key: K, path: Nibble<Vec<u8>>, value: V) -> Option<V>
    where
        Nibble<T>: From<Nibble<Vec<u8>>>,
    {
        debug!("removing leaf");
        let (n, v) = match self.db.remove(&key) {
            Some(Node::Leaf(n, v)) => (n, v),
            _ => return None,
        };
        if path == n.as_slice() {
            self.db.insert(key, Node::Leaf(n, value));
            return Some(v);
        }
        let common = path
            .iter()
            .zip(n.iter())
            .take_while(|(u, v)| u == v)
            .map(|(u, _)| u)
            .collect::<Vec<_>>();

        let mut keys = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        let mut branch_val = None;

        debug!(
            "(path: {}, n: {}, common: {})",
            path.len(),
            n.len(),
            common.len()
        );

        if common.is_empty() {
            // branch then 2 leaves

            if path.len() == 1 {
                debug!("using branch value");
                branch_val = Some(value);
            } else {
                let (i, nibble) = path
                    .as_slice()
                    .split_first()
                    .expect("pos == 0 so there is an item");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), value));
                keys[i as usize] = Some(key);
            }
            if n.len() == 1 {
                debug!("using branch value");
                branch_val = Some(v);
            } else {
                let (i, nibble) = n
                    .as_slice()
                    .split_first()
                    .expect("pos == 0 so there is an item");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), v));
                keys[i as usize] = Some(key);
            }
            debug!("inserting branch");
            self.db.insert(key, Node::Branch(keys, branch_val));
        } else {
            // extension, branch, then 2 leaves
            let start = Nibble::from_nibbles(&common);
            if path.len() == start.len() {
                debug!("using branch value");
                branch_val = Some(value);
            } else {
                let nibble = path
                    .as_slice()
                    .split_n(start.len())
                    .expect("nibble is bigger than start");
                let (i, nibble) = nibble
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), value));
                keys[i as usize] = Some(key);
            }
            if n.len() == start.len() {
                debug!("using branch value");
                branch_val = Some(v);
            } else {
                let nibble = n
                    .as_slice()
                    .split_n(start.len())
                    .expect("nibble is bigger than start");
                let (i, nibble) = nibble
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), v));
                keys[i as usize] = Some(key);
            }
            debug!("pushing branch");
            let branch_key = self.db.push(Node::Branch(keys, branch_val));
            debug!("inserting extension");
            self.db
                .insert(key, Node::Extension(start.into(), branch_key));
        }
        None
    }

    fn break_extension(&mut self, key: K, path: Nibble<Vec<u8>>, value: V) -> Option<V>
    where
        Nibble<T>: From<Nibble<Vec<u8>>>,
    {
        debug!("removing extension");
        let (n, k) = match self.db.remove(&key) {
            Some(Node::Extension(n, k)) => (n, k),
            _ => return None,
        };
        let common = path
            .iter()
            .zip(n.iter())
            .take_while(|(u, v)| u == v)
            .map(|(u, _)| u)
            .collect::<Vec<_>>();

        let mut keys = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        let mut branch_val = None;

        debug!(
            "(path: {}, n: {}, common: {})",
            path.len(),
            n.len(),
            common.len()
        );
        if common.is_empty() {
            // branch then 2 leaves
            if path.len() == 1 {
                debug!("using branch value");
                branch_val = Some(value);
            } else {
                let (i, nibble) = path
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), value));
                keys[i as usize] = Some(key);
            }
            if n.len() == 1 {
                let ext_val = n.iter().next().expect("There is one item");
                debug!("using branch value");
                keys[ext_val as usize] = Some(k);
            } else {
                let (i, nibble) = n
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing extension");
                let key = self.db.push(Node::Extension(nibble.to_vec().into(), k));
                keys[i as usize] = Some(key);
            }
        } else {
            // extension, branch, then 2 leaves
            let start = Nibble::from_nibbles(&common);
            if path.len() == start.len() {
                debug!("using branch value");
                branch_val = Some(value);
            } else {
                let nibble = path
                    .as_slice()
                    .split_n(start.len())
                    .expect("nibble is bigger than start");
                let (i, nibble) = nibble
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing leaf");
                let key = self.db.push(Node::Leaf(nibble.to_vec().into(), value));
                keys[i as usize] = Some(key);
            }
            let nibble = n
                .as_slice()
                .split_n(start.len())
                .expect("nibble is bigger than start");
            let (i, nibble) = nibble
                .as_slice()
                .split_first()
                .expect("nibble is bigger than start");
            debug!("pushing extension");
            let ext_key = self.db.push(Node::Extension(nibble.to_vec().into(), k));
            keys[i as usize] = Some(ext_key);
        }
        debug!("pushing branch");
        self.db.insert(key, Node::Branch(keys, branch_val));
        None
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use storage::{Storage, merkle::MerkleStorage};
    use std::sync::{Once, ONCE_INIT};
    use std::fmt::Debug;

    static INIT: Once = ONCE_INIT;

    type VecStorage = Vec<Option<Node<Vec<u8>, usize, Vec<u8>>>>;

    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            ::env_logger::init();
        });
    }

    fn node_eq<T, K, V, S>(trie: &Trie<S>, kv: Vec<(&str, &str)>)
        where
            S: Storage<Key = K, Value = Node<T, K, V>>,
            T: AsRef<[u8]> + Debug,
            K: Clone + PartialEq,
            V: AsRef<[u8]> + Debug,
    {
        for (k, val) in kv {
            let v = trie.get(Nibble::from_slice(k.as_bytes(), 0));
            assert_eq!(v.map(|v| v.as_ref()), Some(val.as_bytes()));
        }
    }

    #[test]
    fn test_vec() {
        setup();

        let storage: VecStorage = Vec::new();
        let mut trie = Trie::new(storage);

        trie.insert(
            Nibble::from_slice(b"test node", 0).to_vec(),
            "my node".as_bytes().to_vec(),
        );
        node_eq(&trie, vec![("test node", "my node")]);

        trie.insert(
            Nibble::from_slice(b"test", 0).to_vec(),
            "my node short".as_bytes().to_vec(),
        );
        node_eq(
            &trie,
            vec![("test node", "my node"), ("test", "my node short")],
        );

        trie.insert(
            Nibble::from_slice(b"test node 3", 0).to_vec(),
            "my node long".as_bytes().to_vec(),
        );
        node_eq(
            &trie,
            vec![
                ("test node", "my node"),
                ("test", "my node short"),
                ("test node 3", "my node long"),
            ],
        );
    }

    #[test]
    fn test_merkle() {
        setup();

        let storage = MerkleStorage::new();
        let mut trie = Trie::new(storage);

        trie.insert(
            Nibble::from_slice(b"test node", 0).to_vec(),
            "my node".as_bytes().to_vec(),
        );
        node_eq(&trie, vec![("test node", "my node")]);

        trie.insert(
            Nibble::from_slice(b"test", 0).to_vec(),
            "my node short".as_bytes().to_vec(),
        );
        node_eq(
            &trie,
            vec![("test node", "my node"), ("test", "my node short")],
        );

        trie.insert(
            Nibble::from_slice(b"test node 3", 0).to_vec(),
            "my node long".as_bytes().to_vec(),
        );
        node_eq(
            &trie,
            vec![
                ("test node", "my node"),
                ("test", "my node short"),
                ("test node 3", "my node long"),
            ],
        );
    }
}
