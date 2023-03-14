/// A bitfield.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Bitfield {
    pub(crate) raw: Vec<u8>,
}

impl From<Vec<u8>> for Bitfield {
    fn from(raw: Vec<u8>) -> Self {
        Self { raw }
    }
}

impl Bitfield {
    pub fn iter(&self) -> impl Iterator<Item = usize> + Send + Sync + '_
    {
        self.raw.iter()
            .flat_map(|b| {
                (0..8).into_iter().map(move |i| {
                    b & (1 << i) != 0
                })
            })
            .enumerate()
            .filter_map(|(i, v)| if v { Some(i) } else { None })
    }

    pub fn padding_len(&self) -> usize {
        let mut padding = 0;
        for i in (0..self.raw.len()).rev() {
            if self.raw[i] == 0 {
                padding += 1;
            } else {
                break;
            }
        }
        padding
    }

    /// Compares two feature sets for semantic equality.
    pub fn normalized_eq(&self, other: &Self) -> bool {
        let (small, big) = if self.raw.len() < other.raw.len() {
            (self, other)
        } else {
            (other, self)
        };

        for (s, b) in small.raw.iter().zip(big.raw.iter()) {
            if s != b {
                return false;
            }
        }

        for &b in &big.raw[small.raw.len()..] {
            if b != 0 {
                return false;
            }
        }

        true
    }

    /// Returns a slice containing the raw values.
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.raw
    }

    /// Returns whether the specified flag is set.
    pub fn get(&self, bit: usize) -> bool {
        let byte = bit / 8;

        if byte >= self.raw.len() {
            // Unset bits are false.
            false
        } else {
            (self.raw[byte] & (1 << (bit % 8))) != 0
        }
    }

    /// Remove any trailing padding.
    fn clear_padding(mut self) -> Self {
        while !self.raw.is_empty() && self.raw[self.raw.len() - 1] == 0 {
            self.raw.truncate(self.raw.len() - 1);
        }

        self
    }

    /// Sets the specified flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    pub fn set(mut self, bit: usize) -> Self {
        let byte = bit / 8;
        while self.raw.len() <= byte {
            self.raw.push(0);
        }
        self.raw[byte] |= 1 << (bit % 8);

        self.clear_padding()
    }

    /// Clears the specified flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    pub fn clear(mut self, bit: usize) -> Self {
        let byte = bit / 8;
        if byte < self.raw.len() {
            self.raw[byte] &= !(1 << (bit % 8));
        }

        self.clear_padding()
    }
}
