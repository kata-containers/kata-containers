use std::mem;
use std::mem::MaybeUninit;
use std::slice;

/// `Vec::spare_capacity_mut` is not stable until Rust 1.60.
pub(crate) fn vec_spare_capacity_mut<A>(vec: &mut Vec<A>) -> &mut [MaybeUninit<A>] {
    // SAFETY: copy-paste from rust stdlib.
    unsafe {
        slice::from_raw_parts_mut(
            vec.as_mut_ptr().add(vec.len()) as *mut MaybeUninit<A>,
            vec.capacity() - vec.len(),
        )
    }
}

/// `MaybeUninit::write_slice` is not stable.
pub(crate) fn maybe_uninit_write_slice<'a, T>(
    this: &'a mut [MaybeUninit<T>],
    src: &[T],
) -> &'a mut [T]
where
    T: Copy,
{
    // SAFETY: copy-paste from rust stdlib.

    let uninit_src: &[MaybeUninit<T>] = unsafe { mem::transmute(src) };

    this.copy_from_slice(uninit_src);

    unsafe { &mut *(this as *mut [MaybeUninit<T>] as *mut [T]) }
}

/// `MaybeUninit::array_assume_init` is not stable.
#[inline]
pub(crate) unsafe fn maybe_ununit_array_assume_init<T, const N: usize>(
    array: [MaybeUninit<T>; N],
) -> [T; N] {
    // SAFETY:
    // * The caller guarantees that all elements of the array are initialized
    // * `MaybeUninit<T>` and T are guaranteed to have the same layout
    // * `MaybeUninit` does not drop, so there are no double-frees
    // And thus the conversion is safe
    (&array as *const _ as *const [T; N]).read()
}

/// `MaybeUninit::write` is stable since 1.55.
#[inline]
pub(crate) fn maybe_uninit_write<T>(uninit: &mut MaybeUninit<T>, val: T) -> &mut T {
    // SAFETY: copy-paste from rust stdlib.
    *uninit = MaybeUninit::new(val);
    unsafe { &mut *uninit.as_mut_ptr() }
}
