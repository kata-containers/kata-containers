use alloc::vec::Vec;
use byteorder::{ByteOrder, LittleEndian};
use core::iter;
use ff::PrimeField;

use super::Group;

/// Extension trait on a [`Group`] that provides helpers used by [`Wnaf`].
pub trait WnafGroup: Group {
    /// Recommends a wNAF window size given the number of scalars you intend to multiply
    /// a base by. Always returns a number between 2 and 22, inclusive.
    fn recommended_wnaf_for_num_scalars(num_scalars: usize) -> usize;
}

/// Replaces the contents of `table` with a w-NAF window table for the given window size.
pub(crate) fn wnaf_table<G: Group>(table: &mut Vec<G>, mut base: G, window: usize) {
    table.truncate(0);
    table.reserve(1 << (window - 1));

    let dbl = base.double();

    for _ in 0..(1 << (window - 1)) {
        table.push(base);
        base.add_assign(&dbl);
    }
}

/// Replaces the contents of `wnaf` with the w-NAF representation of a little-endian
/// scalar.
pub(crate) fn wnaf_form<S: AsRef<[u8]>>(wnaf: &mut Vec<i64>, c: S, window: usize) {
    // Required by the NAF definition
    debug_assert!(window >= 2);
    // Required so that the NAF digits fit in i64
    debug_assert!(window <= 64);

    wnaf.truncate(0);

    let bit_len = c.as_ref().len() * 8;
    let u64_len = (bit_len + 1) / 64;

    let mut c_u64 = vec![0u64; u64_len + 1];
    LittleEndian::read_u64_into(c.as_ref(), &mut c_u64[0..u64_len]);

    let width = 1u64 << window;
    let window_mask = width - 1;

    let mut pos = 0;
    let mut carry = 0;
    while pos < bit_len {
        // Construct a buffer of bits of the scalar, starting at bit `pos`
        let u64_idx = pos / 64;
        let bit_idx = pos % 64;
        let bit_buf = if bit_idx + window < 64 {
            // This window's bits are contained in a single u64
            c_u64[u64_idx] >> bit_idx
        } else {
            // Combine the current u64's bits with the bits from the next u64
            (c_u64[u64_idx] >> bit_idx) | (c_u64[u64_idx + 1] << (64 - bit_idx))
        };

        // Add the carry into the current window
        let window_val = carry + (bit_buf & window_mask);

        if window_val & 1 == 0 {
            // If the window value is even, preserve the carry and emit 0.
            // Why is the carry preserved?
            // If carry == 0 and window_val & 1 == 0, then the next carry should be 0
            // If carry == 1 and window_val & 1 == 0, then bit_buf & 1 == 1 so the next carry should be 1
            wnaf.push(0);
            pos += 1;
        } else {
            wnaf.push(if window_val < width / 2 {
                carry = 0;
                window_val as i64
            } else {
                carry = 1;
                (window_val as i64).wrapping_sub(width as i64)
            });
            wnaf.extend(iter::repeat(0).take(window - 1));
            pos += window;
        }
    }
}

/// Performs w-NAF exponentiation with the provided window table and w-NAF form scalar.
///
/// This function must be provided a `table` and `wnaf` that were constructed with
/// the same window size; otherwise, it may panic or produce invalid results.
pub(crate) fn wnaf_exp<G: Group>(table: &[G], wnaf: &[i64]) -> G {
    let mut result = G::identity();

    let mut found_one = false;

    for n in wnaf.iter().rev() {
        if found_one {
            result = result.double();
        }

        if *n != 0 {
            found_one = true;

            if *n > 0 {
                result += &table[(n / 2) as usize];
            } else {
                result -= &table[((-n) / 2) as usize];
            }
        }
    }

    result
}

/// A "w-ary non-adjacent form" exponentiation context.
#[derive(Debug)]
pub struct Wnaf<W, B, S> {
    base: B,
    scalar: S,
    window_size: W,
}

impl<G: Group> Wnaf<(), Vec<G>, Vec<i64>> {
    /// Construct a new wNAF context without allocating.
    pub fn new() -> Self {
        Wnaf {
            base: vec![],
            scalar: vec![],
            window_size: (),
        }
    }
}

impl<G: WnafGroup> Wnaf<(), Vec<G>, Vec<i64>> {
    /// Given a base and a number of scalars, compute a window table and return a `Wnaf` object that
    /// can perform exponentiations with `.scalar(..)`.
    pub fn base(&mut self, base: G, num_scalars: usize) -> Wnaf<usize, &[G], &mut Vec<i64>> {
        // Compute the appropriate window size based on the number of scalars.
        let window_size = G::recommended_wnaf_for_num_scalars(num_scalars);

        // Compute a wNAF table for the provided base and window size.
        wnaf_table(&mut self.base, base, window_size);

        // Return a Wnaf object that immutably borrows the computed base storage location,
        // but mutably borrows the scalar storage location.
        Wnaf {
            base: &self.base[..],
            scalar: &mut self.scalar,
            window_size,
        }
    }

    /// Given a scalar, compute its wNAF representation and return a `Wnaf` object that can perform
    /// exponentiations with `.base(..)`.
    pub fn scalar(&mut self, scalar: &<G as Group>::Scalar) -> Wnaf<usize, &mut Vec<G>, &[i64]> {
        // We hard-code a window size of 4.
        let window_size = 4;

        // Compute the wNAF form of the scalar.
        wnaf_form(&mut self.scalar, scalar.to_repr(), window_size);

        // Return a Wnaf object that mutably borrows the base storage location, but
        // immutably borrows the computed wNAF form scalar location.
        Wnaf {
            base: &mut self.base,
            scalar: &self.scalar[..],
            window_size,
        }
    }
}

impl<'a, G: Group> Wnaf<usize, &'a [G], &'a mut Vec<i64>> {
    /// Constructs new space for the scalar representation while borrowing
    /// the computed window table, for sending the window table across threads.
    pub fn shared(&self) -> Wnaf<usize, &'a [G], Vec<i64>> {
        Wnaf {
            base: self.base,
            scalar: vec![],
            window_size: self.window_size,
        }
    }
}

impl<'a, G: Group> Wnaf<usize, &'a mut Vec<G>, &'a [i64]> {
    /// Constructs new space for the window table while borrowing
    /// the computed scalar representation, for sending the scalar representation
    /// across threads.
    pub fn shared(&self) -> Wnaf<usize, Vec<G>, &'a [i64]> {
        Wnaf {
            base: vec![],
            scalar: self.scalar,
            window_size: self.window_size,
        }
    }
}

impl<B, S: AsRef<[i64]>> Wnaf<usize, B, S> {
    /// Performs exponentiation given a base.
    pub fn base<G: Group>(&mut self, base: G) -> G
    where
        B: AsMut<Vec<G>>,
    {
        wnaf_table(self.base.as_mut(), base, self.window_size);
        wnaf_exp(self.base.as_mut(), self.scalar.as_ref())
    }
}

impl<B, S: AsMut<Vec<i64>>> Wnaf<usize, B, S> {
    /// Performs exponentiation given a scalar.
    pub fn scalar<G: Group>(&mut self, scalar: &<G as Group>::Scalar) -> G
    where
        B: AsRef<[G]>,
    {
        wnaf_form(self.scalar.as_mut(), scalar.to_repr(), self.window_size);
        wnaf_exp(self.base.as_ref(), self.scalar.as_mut())
    }
}
