use std::mem;
use std::slice;

/// Slice from `vec[vec.len()..vec.capacity()]`
pub unsafe fn remaining_capacity_as_slice_mut<A>(vec: &mut Vec<A>) -> &mut [A] {
    slice::from_raw_parts_mut(
        vec.as_mut_slice().as_mut_ptr().offset(vec.len() as isize),
        vec.capacity() - vec.len(),
    )
}

pub unsafe fn remove_lifetime_mut<A: ?Sized>(a: &mut A) -> &'static mut A {
    mem::transmute(a)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_remaining_capacity_as_slice_mut() {
        let mut v = Vec::with_capacity(5);
        v.push(10);
        v.push(11);
        v.push(12);
        unsafe {
            {
                let s = remaining_capacity_as_slice_mut(&mut v);
                assert_eq!(2, s.len());
                s[0] = 13;
                s[1] = 14;
            }
            v.set_len(5);
        }
        assert_eq!(vec![10, 11, 12, 13, 14], v);
    }
}
