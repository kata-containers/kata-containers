# take_mut

This crate provides (at this time) a single function, `take()`.

`take()` allows for taking `T` out of a `&mut T`, doing anything with it including consuming it, and producing another `T` to put back in the `&mut T`.

During `take()`, if a panic occurs, the entire process will be exited, as there's no valid `T` to put back into the `&mut T`.

Contrast with `std::mem::replace()`, which allows for putting a different `T` into a `&mut T`, but requiring the new `T` to be available before being able to consume the old `T`.

# Example
```rust
struct Foo;
let mut foo = Foo;
take_mut::take(&mut foo, |foo| {
    // Can now consume the Foo, and provide a new value later
    drop(foo);
    // Do more stuff
    Foo // Return new Foo from closure, which goes back into the &mut Foo
});
```