/// A struct to hold all bytes into the same Vec
#[derive(Debug)]
pub struct Arena {
    data: Vec<u8>,
    pos: Vec<usize>,
}

impl Arena {
    pub fn new() -> Arena {
        Arena {
            data: Vec::new(),
            pos: vec![0],
        }
    }

    pub fn with_capacity(data_cap: usize, item_cap: usize) -> Arena {
        let mut pos = Vec::with_capacity(item_cap + 1);
        pos.push(0);
        Arena {
            data: Vec::with_capacity(data_cap),
            pos,
        }
    }

    pub fn push(&mut self, data: &[u8]) -> usize {
        debug!(
            "pushing data {} (len {}) in arena (len {})",
            self.pos.len(),
            data.len(),
            self.data.len()
        );
        self.data.extend_from_slice(data);
        self.pos.push(self.data.len());
        self.pos.len() - 1
    }

    pub fn insert(&mut self, index: usize, data: &[u8]) {
        debug!(
            "inserting data {} (len {}) at position {} in arena (len {})",
            self.pos.len(),
            data.len(),
            index,
            self.data.len()
        );
        self.data[self.pos[index - 1]..self.pos[index]].copy_from_slice(data);
    }

    pub fn defragment(&mut self, mut used: Vec<usize>) -> Vec<usize> {
        used.sort_unstable();
        used.dedup();
        let mut new_arena = Arena::with_capacity(self.data.len(), self.pos.len());
        let mut out = vec![0; self.pos.len()];
        for i in used {
            out[i] = new_arena.push(&self[i]);
        }
        *self = new_arena;
        out
    }

    pub fn len(&self) -> usize {
        self.pos.len() - 1
    }
}

impl ::std::ops::Index<usize> for Arena {
    type Output = [u8];
    fn index(&self, i: usize) -> &[u8] {
        &self.data[self.pos[i - 1]..self.pos[i]]
    }
}

pub struct ArenaSlice<'a>(pub &'a [&'a [u8]]);

impl<'a> ::std::ops::Index<usize> for ArenaSlice<'a> {
    type Output = [u8];
    fn index(&self, i: usize) -> &[u8] {
        &*self.0[i]
    }
}

#[cfg(test)]
mod test {
    use super::Arena;

    #[test]
    fn arena() {
        let mut arena = Arena::new();
        let idx = arena.push("test".as_bytes());
        let idx2 = arena.push("test2".as_bytes());
        assert_eq!(&arena[idx], "test".as_bytes());
        assert_eq!(&arena[idx2], "test2".as_bytes(), "{:?}", arena);
    }
}
