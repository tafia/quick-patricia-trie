use arena::Arena;
use std::ops::Index;

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

    pub fn iter<'a, A: Index<usize, Output = [u8]>>(
        &'a self,
        arena: &'a A,
    ) -> impl Iterator<Item = u8> + 'a {
        let data = &arena[self.data];
        data.iter()
            .flat_map(|b| Some(b >> 4).into_iter().chain(Some(b & 0x0F).into_iter()))
            .take(self.end)
            .skip(self.start)
    }

    pub fn pop_front<A: Index<usize, Output = [u8]>>(&self, arena: &A) -> Option<(u8, Nibble)> {
        if self.len() == 0 {
            return None;
        }
        let first = arena[self.data][self.start / 2];
        let first = if self.start % 2 == 0 {
            first >> 4
        } else {
            first & 0x0F
        };
        Some((
            first,
            Nibble {
                data: self.data,
                start: self.start + 1,
                end: self.end,
            },
        ))
    }

    pub fn split_at(&self, mut n: usize) -> (Self, Option<Self>) {
        n += self.start;
        if n >= self.end {
            (self.clone(), None)
        } else {
            (
                Nibble { end: n, ..*self },
                Some(Nibble { start: n, ..*self }),
            )
        }
    }

    pub fn eq<A, B>(&self, other: &Self, self_arena: &A, other_arena: &B) -> bool
    where
        A: Index<usize, Output = [u8]>,
        B: Index<usize, Output = [u8]>,
    {
        if self.len() != other.len() {
            return false;
        }
        self.iter(self_arena)
            .zip(other.iter(other_arena))
            .all(|(u, v)| u == v)
    }

    pub fn copy<A>(&self, self_arena: &A, new_arena: &mut Arena) -> Nibble
    where
        A: Index<usize, Output = [u8]>,
    {
        let data = &self_arena[self.data];
        let data = new_arena.push(&data[self.start / 2..]);
        let start = self.start % 2;
        let end = start + self.len();
        Nibble { data, start, end }
    }

    pub fn encoded<A>(&self, is_leaf: bool, arena: &A) -> Vec<u8>
    where
        A: Index<usize, Output = [u8]>,
    {
        let len = self.len();
        let data = &arena[self.data];
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
                buf.push(data[self.start / 2] >> 4 | if is_leaf { 0x30 } else { 0x10 });
                buf.extend(
                    data.windows(2)
                        .take(self.end / 2)
                        .skip(self.start / 2)
                        .map(|w| w[0] << 4 | w[1] >> 4),
                );
            }
            (1, 1) => {
                buf.push(if is_leaf { 0x20 } else { 0 });
                buf.extend(
                    data.windows(2)
                        .take(self.end / 2)
                        .skip(self.start / 2)
                        .map(|w| w[0] << 4 | w[1] >> 4),
                );
            }
            _ => (),
        }
        buf
    }

    /// Decode a slice into a nibble, return true if it is a leaf
    pub fn from_encoded<A>(data: usize, arena: &A) -> (bool, Self)
    where
        A: Index<usize, Output = [u8]>,
    {
        let bytes = &arena[data];
        assert!(!bytes.is_empty(), "Cannot decode empty slice");
        match bytes[0] & 0xF0 {
            0x00 => (
                false,
                Nibble {
                    data,
                    start: 2,
                    end: bytes.len() * 2,
                },
            ),
            0x10 => (
                false,
                Nibble {
                    data,
                    start: 1,
                    end: bytes.len() * 2,
                },
            ),
            0x20 => (
                true,
                Nibble {
                    data,
                    start: 2,
                    end: bytes.len() * 2,
                },
            ),
            0x30 => (
                true,
                Nibble {
                    data,
                    start: 1,
                    end: bytes.len() * 2,
                },
            ),
            s => panic!("Cannot decode slice starting with {:X}", s),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    static D: &'static [u8; 3] = &[0x01u8, 0x23, 0x45];

    #[test]
    fn pop_front() {
        let mut arena = Arena::new();
        let idx = arena.push(&[0x01, 0x23, 0x45]);
        let nibble = Nibble {
            data: idx,
            start: 0,
            end: 6,
        };
        let (i, nibble) = nibble.pop_front(&arena).unwrap();
        assert_eq!(i, 0);
        assert_eq!(
            nibble,
            Nibble {
                data: idx,
                start: 1,
                end: 6,
            }
        );
        let (i, nibble) = nibble.pop_front(&arena).unwrap();
        assert_eq!(i, 1);
        assert_eq!(
            nibble,
            Nibble {
                data: idx,
                start: 2,
                end: 6,
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

    #[test]
    fn encoded() {
        let mut arena = Arena::new();
        let mut n = Nibble::new(D, &mut arena);
        assert_eq!(&n.encoded(false, &arena), &[0x00, 0x01, 0x23, 0x45]);
        assert_eq!(&n.encoded(true, &arena), &[0x20, 0x01, 0x23, 0x45]);
        n.start += 1;
        assert_eq!(&n.encoded(false, &arena), &[0x11, 0x23, 0x45]);
        assert_eq!(&n.encoded(true, &arena), &[0x31, 0x23, 0x45]);
        n.end -= 1;
        assert_eq!(&n.encoded(false, &arena), &[0x00, 0x12, 0x34]);
        assert_eq!(&n.encoded(true, &arena), &[0x20, 0x12, 0x34]);
        n.start += 1;
        assert_eq!(&n.encoded(false, &arena), &[0x12, 0x34]);
        assert_eq!(&n.encoded(true, &arena), &[0x32, 0x34]);
    }

    #[test]
    fn iter_nibble() {
        let mut arena = Arena::new();
        let mut n = Nibble::new(D, &mut arena);
        assert_eq!(n.iter(&arena).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5]);
        n.start += 1;
        assert_eq!(n.iter(&arena).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
        n.end -= 1;
        assert_eq!(n.iter(&arena).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    }
}
