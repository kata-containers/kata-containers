/// An iterator over an Error and its sources.
///
/// If you want to omit the initial error and only process its sources, use `skip(1)`.
///
/// Can be created via [`ErrorCompat::iter_chain`][crate::ErrorCompat::iter_chain].
#[derive(Debug, Clone)]
pub struct ChainCompat<'a> {
    inner: Option<&'a dyn crate::Error>,
}

impl<'a> ChainCompat<'a> {
    /// Creates a new error chain iterator.
    pub fn new(error: &'a dyn crate::Error) -> Self {
        ChainCompat { inner: Some(error) }
    }
}

impl<'a> Iterator for ChainCompat<'a> {
    type Item = &'a dyn crate::Error;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            None => None,
            Some(e) => {
                self.inner = e.source();
                Some(e)
            }
        }
    }
}
