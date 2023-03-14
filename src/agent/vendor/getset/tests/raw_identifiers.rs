#[macro_use]
extern crate getset;

#[derive(CopyGetters, Default, Getters, MutGetters, Setters)]
struct RawIdentifiers {
    #[get]
    r#type: usize,
    #[get_copy]
    r#move: usize,
    #[get_mut]
    r#union: usize,
    #[set]
    r#enum: usize,
    #[get = "with_prefix"]
    r#const: usize,
    #[get_copy = "with_prefix"]
    r#if: usize,
    // Ensure having no gen mode doesn't break things.
    #[allow(dead_code)]
    r#loop: usize,
}

#[test]
fn test_get() {
    let val = RawIdentifiers::default();
    let _ = val.r#type();
}

#[test]
fn test_get_copy() {
    let val = RawIdentifiers::default();
    let _ = val.r#move();
}

#[test]
fn test_get_mut() {
    let mut val = RawIdentifiers::default();
    let _ = val.union_mut();
}

#[test]
fn test_set() {
    let mut val = RawIdentifiers::default();
    val.set_enum(42);
}

#[test]
fn test_get_with_prefix() {
    let val = RawIdentifiers::default();
    let _ = val.get_const();
}

#[test]
fn test_get_copy_with_prefix() {
    let val = RawIdentifiers::default();
    let _ = val.get_if();
}
