use nibbles::Nibble;
use node::{Branch, Extension, Leaf, Node};
use std::marker::PhantomData;
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
pub struct Trie<S, T, K, V> {
    db: S,
    marker_t: PhantomData<T>,
    marker_k: PhantomData<K>,
    marker_v: PhantomData<V>,
}

impl<S, T, K, V> Trie<S, T, K, V> {
    pub fn new(db: S) -> Self {
        Trie {
            db,
            marker_t: PhantomData,
            marker_k: PhantomData,
            marker_v: PhantomData,
        }
    }

    pub fn db(&self) -> &S {
        &self.db
    }
}

impl<S, T, K, V> Trie<S, T, K, V>
where
    S: Storage<T, K, V>,
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
        let mut key = &S::root();
        let mut path = path.as_slice();
        debug!("searching for: {:?}", path);
        loop {
            debug!("path: {:?}", path);
            match self.db.get(key)? {
                Node::Branch(ref branch) => {
                    if let Some((u, n)) = path.split_first() {
                        debug!("got split first");
                        key = branch.get(u)?;
                        path = n;
                    } else {
                        debug!("returning branch value");
                        return branch.get_value();
                    }
                }
                Node::Extension(ref extension) => {
                    path = path.split_start(&extension.nibble().as_slice())?;
                    key = extension.key_ref();
                }
                Node::Leaf(ref leaf) => {
                    debug!("leaf!");
                    return if *leaf.nibble() == path {
                        Some(leaf.value_ref())
                    } else {
                        None
                    };
                }
                Node::Empty => {
                    debug!("empty node!");
                    return None;
                }
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
            let mut key = &S::root();
            let mut path = path.as_slice();
            let is_branch = loop {
                match self.db.get(key)? {
                    Node::Branch(branch) => {
                        if let Some((u, n)) = path.split_first() {
                            key = branch.get(u)?;
                            path = n;
                        } else {
                            break true;
                        }
                    }
                    Node::Extension(extension) => {
                        path = path.split_start(&extension.nibble().as_slice())?;
                        key = extension.key_ref();
                    }
                    Node::Leaf(ref leaf) if *leaf.nibble() == path => break false,
                    _ => return None,
                }
            };
            (is_branch, key.clone())
        };
        if is_branch {
            match self.db.get_mut(&key)? {
                Node::Branch(ref mut branch) => branch.take_value(),
                _ => None,
            }
        } else {
            match self.db.remove(&key)? {
                Node::Leaf(leaf) => Some(leaf.value()),
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
            let mut key = &S::root();
            let action = loop {
                match self.db.get(key) {
                    Some(Node::Branch(branch)) => {
                        if let Some((u, n)) = path.split_first() {
                            path = n;
                            match branch.get(u) {
                                Some(k) => key = k,
                                None => break Action::BranchKey(u),
                            }
                        } else {
                            break Action::BranchValue;
                        }
                    }
                    Some(Node::Extension(extension)) => {
                        match path.split_start(&extension.nibble().as_slice()) {
                            Some(p) => {
                                path = p;
                                key = &extension.key_ref();
                            }
                            None => break Action::BreakExtension,
                        }
                    }
                    Some(Node::Leaf(_)) => break Action::BreakLeaf,
                    Some(Node::Empty) | None => break Action::InsertLeaf,
                }
            };
            (key.clone(), path.to_vec(), action)
        };

        // insert the value with eventually intermediary nodes
        match action {
            Action::InsertLeaf => match self.db.insert_leaf(key, Leaf::new(path.into(), value)) {
                Some(Node::Leaf(leaf)) => Some(leaf.value()),
                _ => None,
            },
            Action::BranchValue => match self.db.get_mut(&key) {
                Some(Node::Branch(ref mut branch)) => {
                    branch.set_value(Some(value));
                    None
                }
                _ => None,
            },
            Action::BranchKey(idx) => {
                let new_key = self.db.push_leaf(Leaf::new(path.into(), value));
                match self.db.get_mut(&key) {
                    Some(Node::Branch(ref mut branch)) => branch.set(idx, Some(new_key)),
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
        let mut leaf = match self.db.remove(&key) {
            Some(Node::Leaf(leaf)) => leaf,
            _ => return None,
        };
        if path == leaf.nibble().as_slice() {
            let value = leaf.set_value(value);
            self.db.insert_leaf(key, leaf);
            return Some(value);
        }
        let common = path
            .iter()
            .zip(leaf.nibble().iter())
            .take_while(|(u, v)| u == v)
            .map(|(u, _)| u)
            .collect::<Vec<_>>();

        let mut branch = Branch::new();

        debug!(
            "(path: {}, n: {}, common: {})",
            path.len(),
            leaf.nibble().len(),
            common.len()
        );

        if common.is_empty() {
            // branch then 2 leaves

            if path.len() == 1 {
                debug!("using branch value");
                branch.set_value(Some(value));
            } else {
                let (i, nibble) = path
                    .as_slice()
                    .split_first()
                    .expect("pos == 0 so there is an item");
                debug!("pushing leaf");
                let key = self.db.push_leaf(Leaf::new(nibble.to_vec().into(), value));
                branch.set(i, Some(key));
            }
            if leaf.nibble().len() == 1 {
                debug!("using branch value");
                branch.set_value(Some(leaf.value()));
            } else {
                let (i, nibble) = {
                    let (i, nibble) = leaf
                        .nibble()
                        .as_slice()
                        .split_first()
                        .expect("pos == 0 so there is an item");
                    (i, nibble.to_vec().into())
                };
                debug!("pushing leaf");
                let key = self.db.push_leaf(Leaf::new(nibble, leaf.value()));
                branch.set(i, Some(key));
            }
            debug!("inserting branch");
            self.db.insert_branch(key, branch);
        } else {
            // extension, branch, then 2 leaves
            let start = Nibble::from_nibbles(&common);
            if path.len() == start.len() {
                debug!("using branch value");
                branch.set_value(Some(value));
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
                let key = self.db.push_leaf(Leaf::new(nibble.to_vec().into(), value));
                branch.set(i, Some(key));
            }
            if leaf.nibble().len() == start.len() {
                debug!("using branch value");
                branch.set_value(Some(leaf.value()));
            } else {
                let (i, nibble) = {
                    let nibble = leaf
                        .nibble()
                        .as_slice()
                        .split_n(start.len())
                        .expect("nibble is bigger than start");
                    let (i, nibble) = nibble
                        .as_slice()
                        .split_first()
                        .expect("nibble is bigger than start");
                    (i, nibble.to_vec().into())
                };
                debug!("pushing leaf");
                leaf.set_nibble(nibble);
                let key = self.db.push_leaf(leaf);
                branch.set(i, Some(key));
            }
            debug!("pushing branch");
            let branch_key = self.db.push_branch(branch);
            debug!("inserting extension");
            self.db
                .insert_extension(key, Extension::new(start.into(), branch_key));
        }
        None
    }

    fn break_extension(&mut self, key: K, path: Nibble<Vec<u8>>, value: V) -> Option<V>
    where
        Nibble<T>: From<Nibble<Vec<u8>>>,
    {
        debug!("removing extension");
        let mut extension = match self.db.remove(&key) {
            Some(Node::Extension(e)) => e,
            _ => return None,
        };
        let common = path
            .iter()
            .zip(extension.nibble().iter())
            .take_while(|(u, v)| u == v)
            .map(|(u, _)| u)
            .collect::<Vec<_>>();

        let mut branch = Branch::new();

        debug!(
            "(path: {}, n: {}, common: {})",
            path.len(),
            extension.nibble().len(),
            common.len()
        );
        if common.is_empty() {
            // branch then 2 leaves
            if path.len() == 1 {
                debug!("using branch value");
                branch.set_value(Some(value));
            } else {
                let (i, nibble) = path
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                debug!("pushing leaf");
                let key = self.db.push_leaf(Leaf::new(nibble.to_vec().into(), value));
                branch.set(i, Some(key));
            }
            if extension.nibble().len() == 1 {
                let ext_val = extension.nibble().iter().next().expect("There is one item");
                debug!("using branch value");
                branch.set(ext_val, Some(extension.key()));
            } else {
                let (i, nibble) = {
                    let (i, nibble) = extension
                        .nibble()
                        .as_slice()
                        .split_first()
                        .expect("nibble is bigger than start");
                    (i, nibble.to_vec().into())
                };
                debug!("pushing extension");
                extension.set_nibble(nibble);
                let key = self.db.push_extension(extension);
                branch.set(i, Some(key));
            }
        } else {
            // extension, branch, then 2 leaves
            let start = Nibble::from_nibbles(&common);
            if path.len() == start.len() {
                debug!("using branch value");
                branch.set_value(Some(value));
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
                let key = self.db.push_leaf(Leaf::new(nibble.to_vec().into(), value));
                branch.set(i, Some(key));
            }
            let (i, nibble) = {
                let nibble = extension
                    .nibble()
                    .as_slice()
                    .split_n(start.len())
                    .expect("nibble is bigger than start");
                let (i, nibble) = nibble
                    .as_slice()
                    .split_first()
                    .expect("nibble is bigger than start");
                (i, nibble.to_vec().into())
            };
            extension.set_nibble(nibble);

            debug!("pushing extension");
            let ext_key = self.db.push_extension(extension);
            branch.set(i, Some(ext_key));
        }
        debug!("pushing branch");
        self.db.insert_branch(key, branch);
        None
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use std::sync::{Once, ONCE_INIT};
    use storage::{merkle::MerkleStorage, VecStorage};

    static INIT: Once = ONCE_INIT;

    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            ::env_logger::init();
        });
    }

    // we use a macro here so the failing test shows where the macro is called instead
    // of the assert_eq line
    macro_rules! node_eq {
        ($trie:expr, $kv:expr) => {
            for (i, (k, val)) in $kv.iter().enumerate() {
                let v = $trie.get(Nibble::from_slice(k.as_bytes(), 0));
                assert_eq!(
                    v.map(|v| v.as_ref()),
                    Some(val.as_bytes()),
                    "error at check {},\n\tk: {:?},\n\tv: {:?}\n\ttrie: {:?}",
                    i + 1,
                    k.as_bytes(),
                    val.as_bytes(),
                    $trie
                );
            }
        };
    }

    #[test]
    fn test_vec() {
        setup();

        let storage: VecStorage<Vec<u8>, Vec<u8>> = Vec::new();
        let mut trie = Trie::new(storage);

        trie.insert(
            Nibble::from_slice(b"test node", 0).to_vec(),
            "my node".as_bytes().to_vec(),
        );
        node_eq!(&trie, vec![("test node", "my node")]);

        trie.insert(
            Nibble::from_slice(b"test", 0).to_vec(),
            "my node short".as_bytes().to_vec(),
        );
        node_eq!(
            &trie,
            vec![("test node", "my node"), ("test", "my node short")]
        );

        trie.insert(
            Nibble::from_slice(b"test node 3", 0).to_vec(),
            "my node long".as_bytes().to_vec(),
        );
        node_eq!(
            &trie,
            vec![
                ("test node", "my node"),
                ("test", "my node short"),
                ("test node 3", "my node long"),
            ]
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
        node_eq!(&trie, vec![("test node", "my node")]);

        trie.insert(
            Nibble::from_slice(b"test", 0).to_vec(),
            "my node short".as_bytes().to_vec(),
        );
        node_eq!(
            &trie,
            vec![("test node", "my node"), ("test", "my node short")]
        );

        trie.insert(
            Nibble::from_slice(b"test node 3", 0).to_vec(),
            "my node long".as_bytes().to_vec(),
        );
        node_eq!(
            &trie,
            vec![
                ("test node", "my node"),
                ("test", "my node short"),
                ("test node 3", "my node long"),
            ]
        );
    }
}
