use nibbles::Nibble;
use node::{Branch, Extension, Leaf, Node};
use std::mem;
use storage::{merkle::MerkleStorage, Arena};

/// A patricia trie
#[derive(Debug)]
pub struct Trie {
    arena: Arena,
    db: MerkleStorage,
}

impl Trie {
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let db = MerkleStorage::new(&mut arena);
        Trie { arena, db }
    }

    pub fn db(&self) -> &MerkleStorage {
        &self.db
    }

    /// Get the item corresponding to that nibble
    pub fn get(&self, mut path: Nibble, arena: &Arena) -> Option<&[u8]> {
        let mut key = self.db.root();
        loop {
            debug!("searching key {:?}", key);
            match self.db.get(&key)? {
                Node::Branch(ref branch) => {
                    if let Some((u, n)) = path.pop_front(arena) {
                        key = branch.keys[u as usize]?;
                        path = n;
                    } else {
                        return branch.value.as_ref().map(|idx| self.arena.get(*idx));
                    }
                }
                Node::Extension(ref extension) => {
                    let (left, right) = path.split_at(extension.nibble.len());
                    if let Some(right) = right {
                        if extension.nibble.eq(&left, &self.arena, Some(arena)) {
                            path = right;
                            key = extension.key;
                            continue;
                        }
                    }
                    return None;
                }
                Node::Leaf(ref leaf) => {
                    return if leaf.nibble.eq(&path, &self.arena, Some(arena)) {
                        Some(self.arena.get(leaf.value))
                    } else {
                        None
                    };
                }
                Node::Empty => return None,
            }
        }
    }

    /// Insert a new leaf
    pub fn insert(&mut self, leaf: Leaf, arena: &Arena) -> Option<&[u8]> {
        let value = self.arena.push(arena.get(leaf.value));
        let mut key = self.db.root();
        let mut path = leaf.nibble;

        enum Action {
            Root,
            BranchKey(u8, Leaf),
            Extension(Extension, usize),
            Leaf(Leaf, usize),
        }

        let action = loop {
            match self.db.get_mut(&key) {
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
                        return old_value.map(move |v| arena.get(v));
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
                        return Some(self.arena.get(old_val));
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
                if let Node::Branch(ref mut branch) = self.db.get_mut(&key)? {
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

    // /// Remove the item corresponding to that nibble
    // pub fn remove(&mut self, path: Nibble) -> Option<&[u8]> {
    //     // In practice we have several scenarii depending on the final node:
    //     // - if node = Leaf && use_empty_node = true => set node as Empty
    //     // - if node = Leaf && use_empty_node = false => remove node from db
    //     // - if node = Branch && value is Some => set value to None
    //     // - if node = Branch && value is None => do nothing
    //     let (is_branch, key) = {
    //         let mut key = &MerkleStorage::root();
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

#[cfg(test)]
mod test {

    use super::*;
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
        ($trie:expr, $leaves:expr, $arena:expr) => {
            for (i, leaf) in $leaves.iter().enumerate() {
                let v = $trie.get(leaf.nibble.clone(), $arena);
                assert_eq!(
                    v,
                    Some($arena.get(leaf.value)),
                    "leaf {}: {:?}\ntrie: {:?}",
                    i,
                    leaf,
                    $trie
                );
            }
        };
    }

    #[test]
    fn trie() {
        setup();

        let mut trie = Trie::new();

        let mut arena = Arena::new();
        let test_leaf = Leaf::new("test node", "my node", &mut arena);
        trie.insert(test_leaf.clone(), &mut arena);
        node_eq!(&trie, vec![&test_leaf], &arena);

        let test_leaf2 = Leaf::new("test", "my node short", &mut arena);
        trie.insert(test_leaf2.clone(), &mut arena);
        node_eq!(&trie, vec![&test_leaf, &test_leaf2], &arena);

        let test_leaf3 = Leaf::new("test node 3", "my node long", &mut arena);
        trie.insert(test_leaf3.clone(), &mut arena);
        node_eq!(&trie, vec![&test_leaf, &test_leaf2, &test_leaf3], &arena);
    }
}
