# maybe-uninit

Quite often, uses of `std::mem::uninitialized()` end up in unsound code.
Therefore, the `MaybeUninit` union has been added to `std::mem` and `std::mem::uninitialized()` is being deprecated.
However, `MaybeUninit` has been added quite recently.
Sometimes you might want to support older versions of Rust as well.
Here is where `maybe-uninit` comes in: it supports stable Rust versions starting with 1.20.0.

Sadly, a feature-complete implementation of `MaybeUninit` is not possible on stable Rust.
Therefore, the library offers the guarantees of `MaybeUninit` in a staged fashion:

* Rust 1.36.0 onward: `MaybeUninit` implementation of Rust stable is being re-exported

* Rust 1.22.x - 1.35.0: No panicing on uninhabited types,
  unsoundness when used with types like `bool` or enums.
  However, there is protection from accidentially `Drop`ing e.g. during unwind!

* Rust 1.20.x - 1.21.x: No support for Copy/Clone of `MaybeUninit<T>`,
  even if `T` impls `Copy` or even `Clone`.
