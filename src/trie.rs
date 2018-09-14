use arena::{Arena, ArenaSlice};
use db::{Db, Index};
use iter::DFSIter;
use nibbles::Nibble;
use node::{Branch, Extension, Leaf, Node};
use std::mem;

/// A patricia trie
#[derive(Debug)]
pub struct Trie {
    arena: Arena,
    db: Db,
}

#[derive(Debug)]
enum Action {
    Root,
    BranchKey(u8, Leaf),
    Extension(Extension, usize),
    Leaf(Leaf, usize),
}

impl Trie {
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let db = Db::new(&mut arena);
        Trie { arena, db }
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    pub(crate) fn arena(&self) -> &Arena {
        &self.arena
    }

    /// Commit all memory node and returns the trie root
    pub fn root(&mut self) -> Option<&[u8]> {
        self.commit();
        self.db.root(&self.arena)
    }

    pub fn get<K: AsRef<[u8]>>(&self, path: K) -> Option<&[u8]> {
        let data = path.as_ref();
        let nibble = Nibble {
            data: 0,
            start: 0,
            end: data.len() * 2,
        };
        let data = &[data];
        let arena = &ArenaSlice(data.as_ref());
        self.get_nibble(nibble, arena)
    }

    /// Get the item corresponding to that nibble
    fn get_nibble<A>(&self, mut path: Nibble, arena: &A) -> Option<&[u8]>
    where
        A: ::std::ops::Index<usize, Output = [u8]>,
    {
        let mut key = self.db.root_index();
        loop {
            debug!("Searching key {:?}", key);
            match self.db.get(&key)? {
                Node::Branch(ref branch) => {
                    debug!("key {:?}: {:?}", key, branch);
                    if let Some((u, n)) = path.pop_front(arena) {
                        key = branch.keys[u as usize]?;
                        path = n;
                    } else {
                        return branch.value.map(|idx| &self.arena[idx]);
                    }
                }
                Node::Extension(ref extension) => {
                    debug!("key {:?}: {:?}", key, extension);
                    let (left, right) = path.split_at(extension.nibble.len());
                    if extension.nibble.eq(&left, &self.arena, arena) {
                        path = right.unwrap_or_default();
                        key = extension.key;
                    } else {
                        return None;
                    }
                }
                Node::Leaf(ref leaf) => {
                    debug!("key {:?}: {:?}", key, leaf);
                    return if leaf.nibble.eq(&path, &self.arena, arena) {
                        Some(&self.arena[leaf.value])
                    } else {
                        warn!("wrong nibble");
                        None
                    };
                }
                Node::Empty => return None,
            }
        }
    }

    pub fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Option<&[u8]> {
        let key = key.as_ref();
        let value = value.as_ref();
        let data = &[key, value];
        let arena = &ArenaSlice(data.as_ref());
        let nibble = Nibble {
            data: 0,
            start: 0,
            end: key.len() * 2,
        };
        let leaf = Leaf { nibble, value: 1 };
        self.insert_leaf(leaf, arena)
    }

    /// Insert a new leaf
    fn insert_leaf<A>(&mut self, leaf: Leaf, arena: &A) -> Option<&[u8]>
    where
        A: ::std::ops::Index<usize, Output = [u8]>,
    {
        let value = self.arena.push(&arena[leaf.value]);
        let mut key = self.db.root_index();
        let mut path = leaf.nibble;

        let action = loop {
            match self.db.get_mut(&mut key) {
                Some(Node::Branch(ref mut branch)) => {
                    if let Some((u, n)) = path.pop_front(arena) {
                        let mut k = branch.keys[u as usize];
                        match k {
                            Some(ref k) => {
                                key = *k;
                                path = n;
                            }
                            None => {
                                // update branch key
                                let nibble = n.copy(arena, &mut self.arena);
                                break Action::BranchKey(u, Leaf { nibble, value });
                            }
                        }
                    } else {
                        // update branch value
                        let old_value = mem::replace(&mut branch.value, Some(value));
                        let arena = &self.arena;
                        return old_value.map(move |v| &arena[v]);
                    }
                }
                Some(Node::Extension(ref extension)) => {
                    let (left, right) = path.split_at(extension.nibble.len());
                    let pos = extension
                        .nibble
                        .iter(&self.arena)
                        .zip(left.iter(arena))
                        .position(|(u, v)| u != v);
                    if let Some(p) = pos {
                        debug!("extension doesn't start with path nor path starts with extension");
                        break Action::Extension(extension.clone(), p);
                    } else {
                        debug!(
                            "path {} starts with extension {}",
                            path.len(),
                            extension.nibble.len()
                        );
                        path = right.unwrap_or_default();
                        key = extension.key;
                    }
                }
                Some(Node::Leaf(ref mut leaf)) => {
                    let (left, right) = path.split_at(leaf.nibble.len());
                    let pos = leaf
                        .nibble
                        .iter(&self.arena)
                        .zip(left.iter(arena))
                        .position(|(u, v)| u != v);
                    if let Some(p) = pos {
                        debug!("leaf doesn't start with path nor path starts with leaf");
                        break Action::Leaf(leaf.clone(), p);
                    } else if let Some(_right) = right {
                        debug!("path starts with leaf (right: {:?})", _right);
                        break Action::Leaf(leaf.clone(), leaf.nibble.len());
                    } else if path.len() == leaf.nibble.len() {
                        debug!("nibble == leaf => replace leaf");
                        let old_val = mem::replace(&mut leaf.value, value);
                        return Some(&self.arena[old_val]);
                    } else {
                        debug!("leaf starts with path");
                        break Action::Leaf(leaf.clone(), path.len());
                    }
                }
                _ => break Action::Root,
            }
        };

        self.execute_action(action, key, value, &path, arena)
    }

    #[inline(always)]
    fn execute_action<A>(
        &mut self,
        action: Action,
        mut key: Index,
        value: usize,
        path: &Nibble,
        arena: &A,
    ) -> Option<&[u8]>
    where
        A: ::std::ops::Index<usize, Output = [u8]>,
    {
        debug!(" -- Inserting {:?}", action);
        match action {
            Action::BranchKey(u, new_leaf) => {
                let new_key = self.db.push_node(Node::Leaf(new_leaf));
                if let Node::Branch(ref mut branch) = self.db.get_mut(&mut key)? {
                    branch.keys[u as usize] = Some(new_key);
                }
            }
            Action::Extension(ext, offset) => {
                self.db.remove(&key);

                let (_, path) = path.split_at(offset);
                let (ext_left, ext_right) = ext.nibble.split_at(offset);

                let mut branch = Branch::default();

                if let Some((u, path)) = path.and_then(|p| p.pop_front(arena)) {
                    let nibble = path.copy(arena, &mut self.arena);
                    let new_key = self.db.push_node(Node::Leaf(Leaf { nibble, value }));
                    branch.keys[u as usize] = Some(new_key);
                } else {
                    branch.value = Some(value);
                }

                if let Some((u, nibble)) = ext_right.and_then(|n| n.pop_front(&self.arena)) {
                    let new_key = if nibble.len() == 0 {
                        // there is no nibble extension so the extension is useless
                        // and we can directly refer to the nibble key
                        ext.key
                    } else {
                        let ext = Extension {
                            nibble,
                            key: ext.key,
                        };
                        self.db.push_node(Node::Extension(ext))
                    };
                    branch.keys[u as usize] = Some(new_key);
                } else {
                    panic!("extension nibble too short");
                }

                if offset > 0 {
                    let branch_key = self.db.push_node(Node::Branch(Box::new(branch)));
                    let ext = Extension {
                        nibble: ext_left,
                        key: branch_key,
                    };
                    self.db.insert_node(key, Node::Extension(ext));
                } else {
                    self.db.insert_node(key, Node::Branch(Box::new(branch)));
                }
            }
            Action::Leaf(leaf, offset) => {
                self.db.remove(&key);
                let mut branch = Branch::default();
                debug!("leaf: {:?}, path: {:?}, offset: {}", leaf, path, offset);
                let (_, path) = path.split_at(offset);
                if let Some((u, path)) = path.and_then(|p| p.pop_front(arena)) {
                    debug!("new leaf: {:?}", path);
                    let nibble = path.copy(arena, &mut self.arena);
                    let new_key = self.db.push_node(Node::Leaf(Leaf { nibble, value }));
                    branch.keys[u as usize] = Some(new_key);
                } else {
                    debug!("new leaf as branch value");
                    branch.value = Some(value);
                }
                let (leaf_left, leaf_right) = leaf.nibble.split_at(offset);
                if let Some((u, nibble)) = leaf_right.and_then(|n| n.pop_front(&self.arena)) {
                    debug!("existing leaf: {:?}", nibble);
                    let leaf = Leaf {
                        nibble,
                        value: leaf.value,
                    };
                    let new_key = self.db.push_node(Node::Leaf(leaf));
                    branch.keys[u as usize] = Some(new_key);
                } else {
                    debug!("existing leaf as branch value");
                    branch.value = Some(leaf.value);
                }
                if offset > 0 {
                    let branch_key = self.db.push_node(Node::Branch(Box::new(branch)));
                    let ext = Extension {
                        nibble: leaf_left,
                        key: branch_key,
                    };
                    self.db.insert_node(key, Node::Extension(ext));
                } else {
                    self.db.insert_node(key, Node::Branch(Box::new(branch)));
                }
            }
            Action::Root => {
                let nibble = path.copy(arena, &mut self.arena);
                self.db.insert_node(key, Node::Leaf(Leaf { nibble, value }));
            }
        }
        None
    }

    pub fn commit(&mut self) {
        self.db.commit(&mut self.arena)
    }

    pub fn iter(&self) -> DFSIter {
        DFSIter::new(self)
    }

    /// Defragment the underlying database
    pub fn defragment(&mut self) {
        self.db.defragment(&mut self.arena);
    }
}

impl Drop for Trie {
    fn drop(&mut self) {
        self.commit();
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use db::Index;
    use keccak_hash::KECCAK_NULL_RLP;
    use keccak_hasher::KeccakHasher;
    use std::str::from_utf8;
    use std::sync::{Once, ONCE_INIT};
    use triehash::trie_root;

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
        ($trie:expr, $inputs:expr) => {
            for (i, &(key, value)) in $inputs.iter().enumerate() {
                let v = $trie.get(key);
                assert_eq!(
                    v,
                    Some(value.as_bytes()),
                    "leaf {}: {:?} / {:?}\ntrie: {:?}",
                    i,
                    key,
                    value,
                    $trie
                );
            }
        };
    }

    #[test]
    fn init() {
        setup();
        let mut trie = Trie::new();
        assert_eq!(trie.root(), Some(KECCAK_NULL_RLP.as_ref()));
    }

    #[test]
    fn insert_on_empty() {
        setup();
        let mut t = Trie::new();

        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        assert_eq!(t.get(&[0x01, 0x23]).unwrap(), &[0x01, 0x23]);

        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![(vec![0x01u8, 0x23], vec![0x01u8, 0x23])]),
        );
    }

    #[test]
    fn insert_replace_root() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01, 0x23].as_ref()));
        t.insert(&[0x01u8, 0x23], &[0x23u8, 0x45]);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x23, 0x45].as_ref()));
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![(vec![0x01u8, 0x23], vec![0x23u8, 0x45])])
        );
    }

    #[test]
    fn insert_make_root() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01, 0x23], &[0x01]);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01].as_ref()));
        t.insert(&[0x01], &[0x02]);
        assert_eq!(t.get(&[0x01]), Some([0x02].as_ref()), "\n{:#?}", t);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01].as_ref()));
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], vec![0x01]),
                (vec![0x01u8], vec![0x02]),
            ])
        );
    }

    #[test]
    fn insert_make_branch_root() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        assert_eq!(t.get(&[0x01, 0x23]).unwrap(), &[0x01, 0x23]);
        t.insert(&[0x11u8, 0x23], &[0x11u8, 0x23]);
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0x11u8, 0x23], vec![0x11u8, 0x23]),
            ])
        );
    }

    #[test]
    fn insert_into_branch_root() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01, 0x23].as_ref()));
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]);
        assert_eq!(t.get(&[0xf1, 0x23]), Some([0xf1, 0x23].as_ref()));
        t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]);
        assert_eq!(
            t.get(&[0x81, 0x23]),
            Some([0x81, 0x23].as_ref()),
            "\n{:?}",
            t
        );
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0x81u8, 0x23], vec![0x81u8, 0x23]),
                (vec![0xf1u8, 0x23], vec![0xf1u8, 0x23]),
            ])
        );
    }

    #[test]
    fn insert_value_into_branch_root() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        t.insert(&[], &[0x0]);
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![], vec![0x0]),
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
            ])
        );
    }

    #[test]
    fn insert_split_leaf() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        t.insert(&[0x01u8, 0x34], &[0x01u8, 0x34]);
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], vec![0x01u8, 0x23]),
                (vec![0x01u8, 0x34], vec![0x01u8, 0x34]),
            ])
        );
    }

    #[test]
    fn insert_split_extension() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01, 0x23, 0x45], &[0x01]);
        assert_eq!(t.get(&[0x01, 0x23, 0x45]), Some([0x01].as_ref()));
        t.insert(&[0x01, 0xf3, 0x45], &[0x02]);
        assert_eq!(t.get(&[0x01, 0xf3, 0x45]), Some([0x02].as_ref()));
        t.insert(&[0x01, 0xf3, 0xf5], &[0x03]);
        assert_eq!(t.get(&[0x01, 0xf3, 0xf5]), Some([0x03].as_ref()));
        t.insert(&[0x01, 0xf3], &[0x04]);
        assert_eq!(t.get(&[0x01, 0xf3]), Some([0x04].as_ref()));
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01, 0x23, 0x45], vec![0x01]),
                (vec![0x01, 0xf3, 0x45], vec![0x02]),
                (vec![0x01, 0xf3, 0xf5], vec![0x03]),
                (vec![0x01, 0xf3], vec![0x04]),
            ])
        );
    }

    #[test]
    fn insert_big_value() {
        let big_value0 = b"00000000000000000000000000000000";
        let big_value1 = b"11111111111111111111111111111111";

        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], big_value0);
        t.insert(&[0x11u8, 0x23], big_value1);
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], big_value0.to_vec()),
                (vec![0x11u8, 0x23], big_value1.to_vec()),
            ])
        );
    }

    #[test]
    fn insert_duplicate_value() {
        let big_value = b"00000000000000000000000000000000";

        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], big_value);
        t.insert(&[0x11u8, 0x23], big_value);
        assert_eq!(
            t.root().unwrap(),
            &*trie_root::<KeccakHasher, _, _, _>(vec![
                (vec![0x01u8, 0x23], big_value.to_vec()),
                (vec![0x11u8, 0x23], big_value.to_vec()),
            ])
        );
    }

    #[test]
    fn test_at_empty() {
        setup();
        let t = Trie::new();
        assert_eq!(t.get(&[0x5]), None);
    }

    #[test]
    fn test_at_one() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        assert_eq!(t.get(&[0x1, 0x23]), Some([0x1u8, 0x23].as_ref()));
        t.commit();
        assert_eq!(t.get(&[0x1, 0x23]), Some([0x1u8, 0x23].as_ref()));
    }

    #[test]
    fn test_at_three() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]);
        t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]);
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0xf1, 0x23]), Some([0xf1u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0x81, 0x23]), Some([0x81u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0x82, 0x23]), None);
        t.commit();
        assert_eq!(t.get(&[0x01, 0x23]), Some([0x01u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0xf1, 0x23]), Some([0xf1u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0x81, 0x23]), Some([0x81u8, 0x23].as_ref()));
        assert_eq!(t.get(&[0x82, 0x23]), None);
    }

    #[test]
    fn trie_basic() {
        setup();

        let mut trie = Trie::new();

        assert_eq!(trie.db.root_index(), Index::Hash(1));

        let inputs = vec![
            ("test node", "my node"),
            ("test", "my node short"),
            ("test node 3", "my node long"),
        ];

        trie.insert(&inputs[0].0, &inputs[0].1);
        node_eq!(&trie, &inputs[..1]);

        trie.insert(&inputs[1].0, &inputs[1].1);
        node_eq!(&trie, &inputs[..2]);

        trie.insert(&inputs[2].0, &inputs[2].1);
        node_eq!(&trie, &inputs[..3]);

        assert_eq!(
            trie.root(),
            Some(
                [
                    239, 218, 198, 132, 179, 205, 251, 214, 82, 69, 141, 191, 115, 22, 225, 130, 4,
                    14, 0, 46, 64, 110, 125, 69, 138, 52, 217, 145, 54, 236, 224, 233
                ]
                    .as_ref()
            ),
        );

        let items = trie.iter().collect::<Vec<_>>();
        'it: for (k1, v1) in items {
            for (k2, v2) in &inputs {
                if v1 == v2.as_bytes() {
                    if k1 != k2.as_bytes() {
                        panic!(
                            "key differ for value '{}':\n'{}' != '{:?}')",
                            v2,
                            k2,
                            from_utf8(&k1)
                        );
                    } else {
                        continue 'it;
                    }
                }
            }
            panic!(
                "Cannot find items ({:?} {:?})",
                from_utf8(&k1),
                from_utf8(v1)
            );
        }
    }

    #[test]
    fn defragment() {
        setup();
        let mut t = Trie::new();
        t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]);
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x23]);
        t.insert(&[0x81u8, 0x23], &[0x81u8, 0x23]);
        t.insert(&[0xf1u8, 0x23], &[0xf1u8, 0x00]);

        t.commit();

        let old_len = t.arena.len();
        t.defragment();
        assert!(old_len > t.arena.len());
    }
}
