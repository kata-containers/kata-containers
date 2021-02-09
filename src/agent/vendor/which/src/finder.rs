use error::*;
#[cfg(windows)]
use helper::has_executable_extension;
use std::env;
use std::ffi::OsStr;
#[cfg(windows)]
use std::ffi::OsString;
use std::iter;
use std::path::{Path, PathBuf};

pub trait Checker {
    fn is_valid(&self, path: &Path) -> bool;
}

trait PathExt {
    fn has_separator(&self) -> bool;

    fn to_absolute<P>(self, cwd: P) -> PathBuf
    where
        P: AsRef<Path>;
}

impl PathExt for PathBuf {
    fn has_separator(&self) -> bool {
        self.components().count() > 1
    }

    fn to_absolute<P>(self, cwd: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        if self.is_absolute() {
            self
        } else {
            let mut new_path = PathBuf::from(cwd.as_ref());
            new_path.push(self);
            new_path
        }
    }
}

pub struct Finder;

impl Finder {
    pub fn new() -> Finder {
        Finder
    }

    pub fn find<T, U, V>(
        &self,
        binary_name: T,
        paths: Option<U>,
        cwd: V,
        binary_checker: &Checker,
    ) -> Result<PathBuf>
    where
        T: AsRef<OsStr>,
        U: AsRef<OsStr>,
        V: AsRef<Path>,
    {
        let path = PathBuf::from(&binary_name);

        let binary_path_candidates: Box<dyn Iterator<Item = _>> = if path.has_separator() {
            // Search binary in cwd if the path have a path separator.
            let candidates = Self::cwd_search_candidates(path, cwd).into_iter();
            Box::new(candidates)
        } else {
            // Search binary in PATHs(defined in environment variable).
            let p = paths.ok_or(ErrorKind::CannotFindBinaryPath)?;
            let paths: Vec<_> = env::split_paths(&p).collect();

            let candidates = Self::path_search_candidates(path, paths).into_iter();

            Box::new(candidates)
        };

        for p in binary_path_candidates {
            // find a valid binary
            if binary_checker.is_valid(&p) {
                return Ok(p);
            }
        }

        // can't find any binary
        return Err(ErrorKind::CannotFindBinaryPath.into());
    }

    fn cwd_search_candidates<C>(binary_name: PathBuf, cwd: C) -> impl IntoIterator<Item = PathBuf>
    where
        C: AsRef<Path>,
    {
        let path = binary_name.to_absolute(cwd);

        Self::append_extension(iter::once(path))
    }

    fn path_search_candidates<P>(
        binary_name: PathBuf,
        paths: P,
    ) -> impl IntoIterator<Item = PathBuf>
    where
        P: IntoIterator<Item = PathBuf>,
    {
        let new_paths = paths.into_iter().map(move |p| p.join(binary_name.clone()));

        Self::append_extension(new_paths)
    }

    #[cfg(unix)]
    fn append_extension<P>(paths: P) -> impl IntoIterator<Item = PathBuf>
    where
        P: IntoIterator<Item = PathBuf>,
    {
        paths
    }

    #[cfg(windows)]
    fn append_extension<P>(paths: P) -> impl IntoIterator<Item = PathBuf>
    where
        P: IntoIterator<Item = PathBuf>,
    {
        // Read PATHEXT env variable and split it into vector of String
        let path_exts =
            env::var_os("PATHEXT").unwrap_or(OsString::from(env::consts::EXE_EXTENSION));

        let exe_extension_vec = env::split_paths(&path_exts)
            .filter_map(|e| e.to_str().map(|e| e.to_owned()))
            .collect::<Vec<_>>();

        paths
            .into_iter()
            .flat_map(move |p| -> Box<dyn Iterator<Item = _>> {
                // Check if path already have executable extension
                if has_executable_extension(&p, &exe_extension_vec) {
                    Box::new(iter::once(p))
                } else {
                    // Appended paths with windows executable extensions.
                    // e.g. path `c:/windows/bin` will expend to:
                    // c:/windows/bin.COM
                    // c:/windows/bin.EXE
                    // c:/windows/bin.CMD
                    // ...
                    let ps = exe_extension_vec.clone().into_iter().map(move |e| {
                        // Append the extension.
                        let mut p = p.clone().to_path_buf().into_os_string();
                        p.push(e);

                        PathBuf::from(p)
                    });

                    Box::new(ps)
                }
            })
    }
}
