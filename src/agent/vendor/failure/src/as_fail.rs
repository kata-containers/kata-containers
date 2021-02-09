use Fail;

/// The `AsFail` trait
///
/// This trait is similar to `AsRef<Fail>`, but it is specialized to handle
/// the dynamic object of `Fail`. Implementors of `Fail` have a blanket
/// implementation. It is used in `failure_derive` in order to generate a
/// custom cause.
pub trait AsFail {
    /// Converts a reference to `Self` into a dynamic trait object of `Fail`.
    fn as_fail(&self) -> &dyn Fail;
}

impl<T> AsFail for T
where
    T: Fail,
{
    fn as_fail(&self) -> &dyn Fail {
        self
    }
}

impl AsFail for dyn Fail {
    fn as_fail(&self) -> &dyn Fail {
        self
    }
}

with_std! {
    use error::Error;

    impl AsFail for Error {
        fn as_fail(&self) -> &dyn Fail {
            self.as_fail()
        }
    }
}
