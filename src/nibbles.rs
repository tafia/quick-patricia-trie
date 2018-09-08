#[derive(Debug)]
pub enum Nibble<T> {
    Even(T),
    Left(u8, T),
}

impl<T: AsRef<[u8]>> Nibble<T> {
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Nibble::Even(ref s) => s.as_ref().len() * 2,
            Nibble::Left(_, ref s) => 1 + s.as_ref().len() * 2,
        }
    }
    #[inline]
    pub fn as_slice<'a>(&'a self) -> Nibble<&'a [u8]> {
        match self {
            Nibble::Even(ref u) => Nibble::Even(u.as_ref()),
            Nibble::Left(l, ref u) => Nibble::Left(*l, u.as_ref()),
        }
    }
    pub fn iter<'a>(&'a self) -> NibbleIter<'a> {
        match self {
            Nibble::Even(ref u) => NibbleIter {
                b: None,
                iter: u.as_ref().iter(),
            },
            Nibble::Left(ref l, ref u) => NibbleIter {
                b: Some(*l),
                iter: u.as_ref().iter(),
            },
        }
    }
}

pub struct NibbleIter<'a> {
    b: Option<u8>,
    iter: ::std::slice::Iter<'a, u8>,
}

impl<'a> Iterator for NibbleIter<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        self.b.take().or_else(|| {
            self.iter.next().and_then(|b| {
                self.b = Some(b & 0x0F);
                Some(b >> 4)
            })
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.iter.size_hint().0 * 2 + if self.b.is_some() { 1 } else { 0 };
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for NibbleIter<'a> {}

impl<U, V> PartialEq<Nibble<U>> for Nibble<V>
where
    U: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    fn eq(&self, other: &Nibble<U>) -> bool {
        match (self, other) {
            (Nibble::Even(u), Nibble::Even(v)) => u.as_ref() == v.as_ref(),
            (Nibble::Left(lu, u), Nibble::Left(lv, v)) => lu == lv && u.as_ref() == v.as_ref(),
            _ => false,
        }
    }
}

impl<T: Default> Default for Nibble<T> {
    fn default() -> Self {
        Nibble::Even(T::default())
    }
}

impl<'a> Nibble<&'a [u8]> {
    #[inline]
    pub fn from_slice(bytes: &'a [u8], start: usize) -> Self {
        if start % 2 == 0 {
            Nibble::Even(&bytes[start / 2..])
        } else {
            Nibble::Left(bytes[start / 2] & 0x0F, &bytes[start / 2 + 1..])
        }
    }

    #[inline]
    pub fn to_vec(&self) -> Nibble<Vec<u8>> {
        match self {
            Nibble::Even(s) => Nibble::Even(s.to_vec()),
            Nibble::Left(l, s) => Nibble::Left(*l, s.to_vec()),
        }
    }

    pub fn split_first(&self) -> Option<(u8, Self)> {
        match self {
            Nibble::Even(s) => {
                if s.is_empty() {
                    None
                } else {
                    Some((s[0] >> 4, Nibble::Left(s[0] & 0x0F, &s[1..])))
                }
            }
            Nibble::Left(l, s) => Some((*l, Nibble::Even(s))),
        }
    }

    pub fn split_start(&self, start: &Self) -> Option<Self> {
        match (self, start) {
            (Nibble::Even(u), Nibble::Even(v)) if u.len() >= v.len() && u.starts_with(v) => {
                return Some(Nibble::Even(&u[v.len()..]));
            }
            (Nibble::Left(lu, u), Nibble::Left(lv, v))
                if u.len() >= v.len() && lu == lv && u.starts_with(v) =>
            {
                return Some(Nibble::Even(&u[v.len()..]));
            }
            (Nibble::Even(u), Nibble::Left(_l, v)) if u.len() > v.len() => {
                if self.iter().zip(start.iter()).all(|(u, v)| u == v) {
                    return Some(Nibble::Left(u[v.len()] & 0x0F, &u[v.len() + 1..]));
                }
            }
            (Nibble::Left(_l, u), Nibble::Even(v)) if u.len() + 1 >= v.len() => {
                if self.iter().zip(start.iter()).all(|(u, v)| u == v) {
                    return Some(Nibble::Left(u[v.len() - 1] & 0x0F, &u[v.len()..]));
                }
            }
            _ => (),
        }
        None
    }

    pub fn split_n(&self, n: usize) -> Option<Self> {
        if n == 0 {
            return match self {
                Nibble::Even(u) => Some(Nibble::Even(*u)),
                Nibble::Left(l, u) => Some(Nibble::Left(*l, *u)),
            };
        }
        match self {
            Nibble::Even(u) => {
                if n >= 2 * u.len() {
                    None
                } else if n % 2 == 0 {
                    Some(Nibble::Even(&u[n / 2..]))
                } else {
                    Some(Nibble::Left(u[n / 2] & 0x0F, &u[n / 2 + 1..]))
                }
            }
            Nibble::Left(_l, u) => {
                if n >= 2 * u.len() + 1 {
                    None
                } else if n % 2 == 1 {
                    Some(Nibble::Even(&u[n / 2..]))
                } else {
                    Some(Nibble::Left(u[n / 2] & 0x0F, &u[n / 2 + 1..]))
                }
            }
        }
    }

    pub fn encode(&self, is_leaf: bool, buf: &mut Vec<u8>) {
        match self {
            Nibble::Even(ref u) => {
                buf.reserve(u.len() + 1);
                buf.push(if is_leaf { 0x20 } else { 0 });
                buf.extend_from_slice(u);
            }
            Nibble::Left(l, ref u) => {
                buf.reserve(u.len() + 1);
                buf.push(l | if is_leaf { 0x30 } else { 0x10 });
                buf.extend_from_slice(u);
            }
        }
    }

    pub fn encoded(&self, is_leaf: bool) -> Vec<u8> {
        match self {
            Nibble::Even(ref u) => {
                let mut buf = Vec::with_capacity(u.len() + 1);
                buf.push(if is_leaf { 0x20 } else { 0 });
                buf.extend_from_slice(u);
                buf
            }
            Nibble::Left(l, ref u) => {
                let mut buf = Vec::with_capacity(u.len() + 1);
                buf.push(l | if is_leaf { 0x30 } else { 0x10 });
                buf.extend_from_slice(u);
                buf
            }
        }
    }

    /// Decode slice, returns a Nibble and a flag if it is a leaf or not
    pub fn decode(data: &'a [u8]) -> (bool, Self) {
        assert!(!data.is_empty(), "Cannot decode empty slice");
        match data[0] & 0xF0 {
            0x00 => (false, Nibble::Even(&data[1..])),
            0x10 => (false, Nibble::Left(data[0] & 0xF0, &data[1..])),
            0x20 => (true, Nibble::Even(&data[1..])),
            0x30 => (true, Nibble::Left(data[0] & 0xF0, &data[1..])),
            s => panic!("Cannot decode slice starting with {:X}", s),
        }
    }
}

impl Nibble<Vec<u8>> {
    pub fn from_vec(mut vec: Vec<u8>, start: usize) -> Nibble<Vec<u8>> {
        if start % 2 == 0 {
            let _ = vec.drain(..start / 2);
            Nibble::Even(vec)
        } else {
            let l = vec
                .drain(..start / 2 + 1)
                .last()
                .expect("start is odd so there is at least one element");
            Nibble::Left(l & 0x0F, vec)
        }
    }
    pub fn from_nibbles(iter: &[u8]) -> Nibble<Vec<u8>> {
        let start = iter.len() % 2;
        if start == 0 {
            Nibble::Even(iter.chunks(2).map(|w| w[0] << 4 | w[1]).collect())
        } else {
            Nibble::Left(
                iter[0],
                iter[1..].chunks(2).map(|w| w[0] << 4 | w[1]).collect(),
            )
        }
    }
}

impl<'a> From<&'a Nibble<Vec<u8>>> for Nibble<&'a [u8]> {
    fn from(nibble: &'a Nibble<Vec<u8>>) -> Self {
        nibble.as_slice()
    }
}

impl<'a, T: AsRef<[u8]>> From<&'a T> for Nibble<&'a [u8]> {
    fn from(slice: &'a T) -> Self {
        Nibble::from_slice(slice.as_ref(), 0)
    }
}
