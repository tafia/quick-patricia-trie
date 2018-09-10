use storage::Arena;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Nibble {
    pub data: usize,
    pub start: usize,
    pub end: usize,
}

impl Nibble {
    pub fn new<D: AsRef<[u8]>>(data: D, arena: &mut Arena) -> Nibble {
        let d = data.as_ref();
        let data = arena.push(d);
        Nibble {
            data,
            start: 0,
            end: d.len() * 2,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn iter<'a>(&'a self, arena: &'a Arena) -> impl Iterator<Item = u8> + 'a {
        let data = arena.get(self.data);
        data[self.start / 2..self.end / 2 + self.end % 2]
            .iter()
            .flat_map(|b| Some(b >> 4).into_iter().chain(Some(b & 0x0F).into_iter()))
            .skip(self.start % 1)
            .take(self.len())
    }

    pub fn pop_front(&self, arena: &Arena) -> Option<(u8, Nibble)> {
        if self.len() == 0 {
            return None;
        }
        let data = arena.get(self.data);
        let first = data[self.start / 2];
        if self.start % 2 == 0 {
            Some((
                first >> 4,
                Nibble {
                    data: self.data,
                    start: self.start + 1,
                    end: self.end,
                },
            ))
        } else {
            Some((
                first & 0x0F,
                Nibble {
                    data: self.data,
                    start: self.start + 1,
                    end: self.end,
                },
            ))
        }
    }

    pub fn split_at(&self, n: usize) -> (Self, Option<Self>) {
        let n = self.start + n;
        if n > self.end {
            (self.clone(), None)
        } else {
            (
                Nibble { end: n, ..*self },
                Some(Nibble { start: n, ..*self }),
            )
        }
    }

    pub fn eq(&self, other: &Self, self_arena: &Arena, other_arena: Option<&Arena>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if other_arena.is_none() && self == other {
            return true;
        }
        self.iter(self_arena)
            .zip(other.iter(other_arena.unwrap_or(self_arena)))
            .all(|(u, v)| u == v)
    }

    pub fn copy(&self, self_arena: &Arena, new_arena: &mut Arena) -> Nibble {
        let data = self_arena.get(self.data);
        let data = new_arena.push(&data[self.start / 2..]);
        let start = self.start % 2;
        let end = start + self.len();
        Nibble { data, start, end }
    }

    pub fn encoded(&self, is_leaf: bool, arena: &Arena) -> Vec<u8> {
        let len = self.len();
        let data = arena.get(self.data);
        let mut buf = Vec::with_capacity(len / 2 + 1);
        match (self.start % 2, self.end % 2) {
            (0, 0) => {
                buf.push(if is_leaf { 0x20 } else { 0 });
                buf.extend_from_slice(&data[self.start / 2..self.end / 2]);
            }
            (1, 0) => {
                buf.push(data[self.start / 2] & 0x0F | if is_leaf { 0x30 } else { 0x10 });
                buf.extend_from_slice(&data[self.start / 2 + 1..self.end / 2]);
            }
            (0, 1) => {
                buf.push(data[self.start / 2] & 0x0F | if is_leaf { 0x30 } else { 0x10 });
                let mut i = self.start;
                while i < self.end {
                    let b = data[i] << 4 | data[i + 1] >> 4;
                    buf.push(b);
                    i += 2;
                }
                buf.push(data[i] << 4);
            }
            (1, 1) => {
                buf.push(if is_leaf { 0x20 } else { 0 });
                let mut i = self.start;
                while i < self.end {
                    let b = data[i] << 4 | data[i + 1] >> 4;
                    buf.push(b);
                    i += 2;
                }
            }
            _ => (),
        }
        buf
    }

    // pub fn decode(data: &'a [u8]) -> (bool, Self) {
    //     unimplemented!()
    //     // assert!(!data.is_empty(), "Cannot decode empty slice");
    //     // match data[0] & 0xF0 {
    //     //     0x00 => (false, Nibble::Even(&data[1..])),
    //     //     0x10 => (false, Nibble::Left(data[0] & 0xF0, &data[1..])),
    //     //     0x20 => (true, Nibble::Even(&data[1..])),
    //     //     0x30 => (true, Nibble::Left(data[0] & 0xF0, &data[1..])),
    //     //     s => panic!("Cannot decode slice starting with {:X}", s),
    //     // }
    // }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pop_front() {
        let mut arena = Arena::new();
        let idx = arena.push("test".as_bytes());
        let nibble = Nibble {
            data: idx,
            start: 0,
            end: "test".len() * 2,
        };
        let (i, nibble2) = nibble.pop_front(&arena).unwrap();
        assert_eq!(i, b't' >> 4);
        assert_eq!(
            nibble2,
            Nibble {
                data: idx,
                start: 1,
                end: nibble.end
            }
        );
    }

    #[test]
    fn split_at() {
        let mut arena = Arena::new();
        let idx = arena.push("test".as_bytes());
        let nibble = Nibble {
            data: idx,
            start: 0,
            end: "test".len() * 2,
        };
        let (left, right) = nibble.split_at(4);
        assert_eq!(left, Nibble { end: 4, ..nibble });
        assert_eq!(right.unwrap(), Nibble { start: 4, ..nibble });
    }
}
