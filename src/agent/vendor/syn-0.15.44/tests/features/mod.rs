#[allow(unused_macros)]
macro_rules! hide_from_rustfmt {
    ($mod:item) => {
        $mod
    };
}

#[cfg(not(all(
    feature = "derive",
    feature = "full",
    feature = "parsing",
    feature = "printing",
    feature = "visit",
    feature = "visit-mut",
    feature = "fold",
    feature = "clone-impls",
    feature = "extra-traits",
    feature = "proc-macro",
)))]
hide_from_rustfmt! {
    mod error;
}
