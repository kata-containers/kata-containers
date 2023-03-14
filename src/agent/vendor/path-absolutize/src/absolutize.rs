use std::borrow::Cow;
use std::io;
use std::path::Path;

/// Let `Path` and `PathBuf` have `absolutize` and `absolutize_virtually` method.
pub trait Absolutize {
    /// Get an absolute path. This works even if the path does not exist.
    fn absolutize(&self) -> io::Result<Cow<Path>>;

    /// Get an absolute path. This works even if the path does not exist. It gets the current working directory as the second argument.
    fn absolutize_from(&self, cwd: &Path) -> io::Result<Cow<'_, Path>>;

    /// Get an absolute path. This works even if the path does not exist.
    fn absolutize_virtually<P: AsRef<Path>>(&self, virtual_root: P) -> io::Result<Cow<Path>>;
}
