use nibbles::Nibble;
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
        let mut start = true;
        let mut nibble = Nibble::default();
        let mut buffer = Vec::new();
        let mut count = 0;
        for n in self.stack.iter() {
            if let NodeIter::Extension(e) = n {
                if start {
                    nibble = e.nibble.clone();
                    nibble.start -= count;
                    start = false;
                } else {
                    if nibble.data == e.nibble.data {
                        if nibble.end + count == e.nibble.start {
                            nibble.end = e.nibble.end;
                        } else {
                            panic!("getting 2 chunks of same nibble??");
                        }
                    } else {
                        let data1 = self.trie.arena().get(nibble.data);
                        let data2 = self.trie.arena().get(e.nibble.data);
                        buffer.extend_from_slice(&data1[nibble.start / 2..nibble.end / 2]);
                        if nibble.end % 2 == 1 && (e.nibble.start - count) % 2 == 1 {
                            buffer.push(
                                data1[nibble.end / 2] & 0xF0
                                    | data2[(e.nibble.start - count) / 2] & 0x0F,
                            );
                            nibble = e.nibble.clone();
                            nibble.start += 1;
                        } else {
                            nibble = e.nibble.clone();
                        }
                        nibble.start -= count;
                    }
                }
                count = 0;
            } else {
                count += 1;
            }
        }

        if let Some(leaf) = leaf {
            let data = self.trie.arena().get(leaf.nibble.data);
            if start {
                Cow::Borrowed(data)
            } else if leaf.nibble.data == nibble.data {
                if buffer.is_empty() {
                    Cow::Borrowed(data)
                } else {
                    buffer.extend_from_slice(&data[nibble.start / 2..nibble.end / 2]);
                    Cow::Owned(buffer)
                }
            } else {
                let data1 = self.trie.arena().get(nibble.data);
                buffer.extend_from_slice(&data1[nibble.start / 2..nibble.end / 2]);
                if nibble.end % 2 == 1 && (leaf.nibble.start - count) % 2 == 1 {
                    buffer.push(
                        data1[nibble.end / 2] & 0xF0 | data[(leaf.nibble.start - count) / 2] & 0x0F,
                    );
                }
                buffer
                    .extend_from_slice(&data[(leaf.nibble.start - count) / 2..leaf.nibble.end / 2]);
                Cow::Owned(buffer)
            }
        } else {
            let data1 = self.trie.arena().get(nibble.data);
            buffer.extend_from_slice(&data1[nibble.start / 2..nibble.end / 2]);
            Cow::Owned(buffer)
        }
    }

    fn branch_item(&self, value: usize) -> (Cow<'a, [u8]>, &'a [u8]) {
        (self.build_key(None), self.trie.arena().get(value))
    }

    fn leaf_item(&mut self, leaf: &'a Leaf) -> (Cow<'a, [u8]>, &'a [u8]) {
        (
            self.build_key(Some(leaf)),
            self.trie.arena().get(leaf.value),
        )
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
                        if let Some(p) = branch
                            .keys
                            .iter()
                            .skip(n.map_or(0, |n| n as usize + 1))
                            .position(|k| k.is_some())
                        {
                            self.stack.push(NodeIter::Branch(branch, Some(p as u8)));
                            break branch.keys[p]?;
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
