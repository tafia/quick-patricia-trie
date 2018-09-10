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
    pub fn get(&self, i: usize) -> &[u8] {
        &self.data[self.pos[i - 1]..self.pos[i]]
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
}

#[cfg(test)]
mod test {
    use super::Arena;

    #[test]
    fn arena() {
        let mut arena = Arena::new();
        let idx = arena.push("test".as_bytes());
        let idx2 = arena.push("test2".as_bytes());
        assert_eq!(arena.get(idx), "test".as_bytes());
        assert_eq!(arena.get(idx2), "test2".as_bytes(), "{:?}", arena);
    }
}
