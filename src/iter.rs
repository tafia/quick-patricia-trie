use node::{Branch, Extension, Leaf, Node};
use std::borrow::Cow;
use trie::Trie;

/// A Depth First Search iterator
///
/// Early stops if a node has not been commited
pub struct DFSIter<'a> {
    stack: Vec<NodeIter<'a>>,
    trie: &'a Trie,
    root: bool,
}

enum NodeIter<'a> {
    Branch(&'a Branch, Option<u8>),
    Extension(&'a Extension),
}

impl<'a> DFSIter<'a> {
    pub fn new(trie: &'a Trie) -> Self {
        DFSIter {
            stack: Vec::new(),
            root: true,
            trie,
        }
    }

    fn build_key(&self, leaf: Option<&Leaf>) -> Cow<'a, [u8]> {
        let mut buffer = Vec::with_capacity(64);
        for n in self.stack.iter() {
            match n {
                NodeIter::Branch(_, Some(n)) => {
                    debug!("one branch");
                    buffer.push(*n);
                }
                NodeIter::Extension(e) => {
                    debug!("one extension {}", e.nibble.len());
                    buffer.extend(e.nibble.iter(self.trie.arena()));
                }
                _ => (),
            }
        }
        if let Some(leaf) = leaf {
            buffer.extend(leaf.nibble.iter(self.trie.arena()));
        }
        debug!("buffer len: {}", buffer.len());
        debug!("buffer {:?}", buffer);
        Cow::Owned(buffer.chunks(2).map(|w| w[0] << 4 | w[1]).collect())
    }

    fn branch_item(&self, value: usize) -> (Cow<'a, [u8]>, &'a [u8]) {
        debug!("getting branch item");
        (self.build_key(None), &self.trie.arena()[value])
    }

    fn leaf_item(&mut self, leaf: &'a Leaf) -> (Cow<'a, [u8]>, &'a [u8]) {
        debug!("getting leaf item");
        (self.build_key(Some(leaf)), &self.trie.arena()[leaf.value])
    }
}

impl<'a> Iterator for DFSIter<'a> {
    type Item = (Cow<'a, [u8]>, &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        let mut key = if self.root {
            self.root = false;
            self.trie.db().root_index()
        } else {
            // search up the stack for the next branch key
            loop {
                match self.stack.pop()? {
                    NodeIter::Branch(branch, n) => {
                        let start = n.map_or(0, |n| n as usize + 1);
                        if let Some(p) = branch.keys.iter().skip(start).position(|k| k.is_some()) {
                            self.stack
                                .push(NodeIter::Branch(branch, Some((start + p) as u8)));
                            break branch.keys[start + p]?;
                        }
                    }
                    NodeIter::Extension(_) => (),
                }
            }
        };

        loop {
            debug!("iter {:?}", key);
            match self.trie.db().get(&key)? {
                Node::Leaf(ref leaf) => return Some(self.leaf_item(leaf)),
                Node::Extension(ref extension) => {
                    self.stack.push(NodeIter::Extension(&extension));
                    key = extension.key;
                }
                Node::Branch(ref branch) => {
                    self.stack.push(NodeIter::Branch(branch, None));
                    return if let Some(v) = branch.value {
                        Some(self.branch_item(v))
                    } else {
                        self.next()
                    };
                }
                Node::Empty => {
                    warn!("found empty node");
                    return None;
                }
            }
        }
    }
}
