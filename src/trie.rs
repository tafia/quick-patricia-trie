use arena::{Arena, ArenaSlice};
use db::Db;
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

impl Trie {
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let db = Db::new(&mut arena);
        Trie { arena, db }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    pub fn root(&self) -> Option<&[u8]> {
        self.db.root(&self.arena)
    }

    pub fn get<A: AsRef<[u8]>>(&self, path: A) -> Option<&[u8]> {
        let data = path.as_ref();
        let nibble = Nibble {
            data: 0,
            start: 0,
            end: data.len(),
        };
        let data = &[data];
        let arena = &ArenaSlice(data.as_ref());
        self.get_nibble(nibble, arena)
    }

    /// Get the item corresponding to that nibble
    pub fn get_nibble<A>(&self, mut path: Nibble, arena: &A) -> Option<&[u8]>
    where
        A: ::std::ops::Index<usize, Output = [u8]>,
    {
        let mut key = self.db.root_index();
        loop {
            debug!("searching key {:?}", key);
            match self.db.get(&key)? {
                Node::Branch(ref branch) => {
                    if let Some((u, n)) = path.pop_front(arena) {
                        key = branch.keys[u as usize]?;
                        path = n;
                    } else {
                        return branch.value.as_ref().map(|idx| &self.arena[*idx]);
                    }
                }
                Node::Extension(ref extension) => {
                    let (left, right) = path.split_at(extension.nibble.len());
                    if let Some(right) = right {
                        if extension.nibble.eq(&left, &self.arena, arena) {
                            path = right;
                            key = extension.key;
                            continue;
                        }
                    }
                    return None;
                }
                Node::Leaf(ref leaf) => {
                    return if leaf.nibble.eq(&path, &self.arena, arena) {
                        Some(&self.arena[leaf.value])
                    } else {
                        None
                    };
                }
                Node::Empty => return None,
            }
        }
    }

    pub fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Option<&[u8]> {
        let key = key.as_ref();
        let nibble = Nibble {
            data: 0,
            start: 0,
            end: key.len(),
        };
        let value = value.as_ref();
        let data = &[key, value];
        let arena = &ArenaSlice(data.as_ref());
        let leaf = Leaf { nibble, value: 1 };
        self.insert_leaf(leaf, arena)
    }

    /// Insert a new leaf
    pub fn insert_leaf<A>(&mut self, leaf: Leaf, arena: &A) -> Option<&[u8]>
    where
        A: ::std::ops::Index<usize, Output = [u8]>,
    {
        let value = self.arena.push(&arena[leaf.value]);
        let mut key = self.db.root_index();
        let mut path = leaf.nibble;

        enum Action {
            Root,
            BranchKey(u8, Leaf),
            Extension(Extension, usize),
            Leaf(Leaf, usize),
        }

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
                                let nibble = path.copy(arena, &mut self.arena);
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
                        // extension doesn't start with path nor path starts with extension
                        break Action::Extension(extension.clone(), p);
                    } else if let Some(right) = right {
                        // path starts with extension
                        path = right;
                        key = extension.key;
                    } else {
                        // extension starts with path
                        break Action::Extension(extension.clone(), path.len());
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
                        // leaf doesn't start with path nor path starts with leaf
                        break Action::Leaf(leaf.clone(), p);
                    } else if let Some(_right) = right {
                        // path starts with leaf
                        break Action::Leaf(leaf.clone(), leaf.nibble.len());
                    } else if path.len() == leaf.nibble.len() {
                        // exact same nibble => replace leaf
                        let old_val = mem::replace(&mut leaf.value, value);
                        return Some(&self.arena[old_val]);
                    } else {
                        // leaf starts with path
                        break Action::Leaf(leaf.clone(), path.len());
                    }
                }
                _ => break Action::Root,
            }
        };
        match action {
            Action::BranchKey(u, new_leaf) => {
                let new_key = self.db.push_leaf(new_leaf);
                if let Node::Branch(ref mut branch) = self.db.get_mut(&mut key)? {
                    branch.keys[u as usize] = Some(new_key);
                }
            }
            Action::Extension(ext, offset) => {
                self.db.remove(&key);
                let mut branch = Branch::new();
                if offset == 0 {
                    if let Some((u, path)) = path.pop_front(arena) {
                        let nibble = path.copy(arena, &mut self.arena);
                        let new_key = self.db.push_leaf(Leaf { nibble, value });
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(value);
                    }
                    let (u, nibble) = ext
                        .nibble
                        .pop_front(&self.arena)
                        .expect("we are explicitly checking NOT to create empty nibble extension");
                    let new_key = if nibble.len() == 0 {
                        // there is no nibble extension so the extension is useless
                        // and we can directly refer to the nibble key
                        ext.key
                    } else {
                        let ext = Extension {
                            nibble,
                            key: ext.key,
                        };
                        self.db.push_extension(ext)
                    };
                    branch.keys[u as usize] = Some(new_key);
                    self.db.insert_node(key, Node::Branch(branch));
                } else {
                    let (_, path) = path.split_at(offset);
                    if let Some((u, path)) = path.and_then(|p| p.pop_front(arena)) {
                        let nibble = path.copy(arena, &mut self.arena);
                        let new_key = self.db.push_leaf(Leaf { nibble, value });
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(value);
                    }

                    let (ext_left, ext_right) = ext.nibble.split_at(offset);

                    let (u, nibble) = ext_right
                        .and_then(|n| n.pop_front(&self.arena))
                        .expect("extension is bigger than offset because we are spliting it");
                    let new_key = if nibble.len() == 0 {
                        // there is no nibble extension so the extension is useless
                        // and we can directly refer to the nibble key
                        ext.key
                    } else {
                        let ext = Extension {
                            nibble,
                            key: ext.key,
                        };
                        self.db.push_extension(ext)
                    };
                    branch.keys[u as usize] = Some(new_key);
                    let branch_key = self.db.push_branch(branch);

                    let ext = Extension {
                        nibble: ext_left,
                        key: branch_key,
                    };
                    self.db.insert_node(key, Node::Extension(ext));
                }
            }
            Action::Leaf(leaf, offset) => {
                self.db.remove(&key);
                let mut branch = Branch::new();
                if offset == 0 {
                    if let Some((u, path)) = path.pop_front(arena) {
                        let nibble = path.copy(arena, &mut self.arena);
                        let new_key = self.db.push_leaf(Leaf { nibble, value });
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(value);
                    }
                    if let Some((u, nibble)) = leaf.nibble.pop_front(&self.arena) {
                        let leaf = Leaf {
                            nibble,
                            value: leaf.value,
                        };
                        let new_key = self.db.push_leaf(leaf);
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(leaf.value);
                    }
                    self.db.insert_node(key, Node::Branch(branch));
                } else {
                    let (_, path) = path.split_at(offset);
                    if let Some((u, path)) = path.and_then(|p| p.pop_front(arena)) {
                        let nibble = path.copy(arena, &mut self.arena);
                        let new_key = self.db.push_leaf(Leaf { nibble, value });
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(value);
                    }
                    let (leaf_left, leaf_right) = leaf.nibble.split_at(offset);
                    if let Some((u, nibble)) = leaf_right.and_then(|n| n.pop_front(&self.arena)) {
                        let leaf = Leaf {
                            nibble,
                            value: leaf.value,
                        };
                        let new_key = self.db.push_leaf(leaf);
                        branch.keys[u as usize] = Some(new_key);
                    } else {
                        branch.value = Some(leaf.value);
                    }
                    let branch_key = self.db.push_branch(branch);
                    let ext = Extension {
                        nibble: leaf_left,
                        key: branch_key,
                    };
                    self.db.insert_node(key, Node::Extension(ext));
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

    // /// Remove the item corresponding to that nibble
    // pub fn remove(&mut self, path: Nibble) -> Option<&[u8]> {
    //     // In practice we have several scenarii depending on the final node:
    //     // - if node = Leaf && use_empty_node = true => set node as Empty
    //     // - if node = Leaf && use_empty_node = false => remove node from db
    //     // - if node = Branch && value is Some => set value to None
    //     // - if node = Branch && value is None => do nothing
    //     let (is_branch, key) = {
    //         let mut key = &Db::root_index();
    //         let mut path = path.as_slice();
    //         let is_branch = loop {
    //             match self.db.get(key)? {
    //                 Node::Branch(branch) => {
    //                     if let Some((u, n)) = path.split_first() {
    //                         key = branch.get(u)?;
    //                         path = n;
    //                     } else {
    //                         break true;
    //                     }
    //                 }
    //                 Node::Extension(extension) => {
    //                     path = path.split_start(&extension.nibble().as_slice())?;
    //                     key = extension.key_ref();
    //                 }
    //                 Node::Leaf(ref leaf) if *leaf.nibble() == path => break false,
    //                 _ => return None,
    //             }
    //         };
    //         (is_branch, key.clone())
    //     };
    //     if is_branch {
    //         match self.db.get_mut(&key)? {
    //             Node::Branch(ref mut branch) => branch.take_value(),
    //             _ => None,
    //         }
    //     } else {
    //         match self.db.remove(&key)? {
    //             Node::Leaf(leaf) => Some(leaf.value()),
    //             _ => None,
    //         }
    //     }
    // }
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
    use std::str::from_utf8;
    use std::sync::{Once, ONCE_INIT};

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
        let trie = Trie::new();
        assert_eq!(trie.root(), Some(KECCAK_NULL_RLP.as_ref()));
    }

    // #[test]
    // fn insert_on_empty() {
    //     let mut trie = Trie::new();
    //     t.insert(&[0x01u8, 0x23], &[0x01u8, 0x23]).unwrap();
    //     assert_eq!(*t.root(), trie_root::<KeccakHasher, _, _, _>(vec![ (vec![0x01u8, 0x23], vec![0x01u8, 0x23]) ]));
    // }

    #[test]
    fn trie() {
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
        assert_eq!(trie.root(), None);

        trie.insert(&inputs[1].0, &inputs[1].1);
        node_eq!(&trie, &inputs[..2]);
        assert_eq!(trie.root(), None);

        trie.insert(&inputs[2].0, &inputs[2].1);
        node_eq!(&trie, &inputs[..3]);
        assert_eq!(trie.root(), None);

        trie.commit();
        assert_eq!(
            trie.root(),
            Some(
                [
                    109, 28, 40, 33, 242, 196, 136, 177, 223, 75, 161, 203, 167, 31, 110, 63, 207,
                    41, 70, 85, 75, 148, 236, 235, 16, 176, 214, 117, 97, 91, 48, 212
                ].as_ref() // [55, 30, 154, 189, 178, 144, 235, 49, 56, 30, 179, 45, 122, 76, 77, 4, 177, 6, 166, 164, 65, 4, 191, 80, 163, 159, 104, 211, 120, 125, 101, 60].as_ref()
            ),
            "{:?}",
            trie
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
}
