use std::{
    env,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{CommandExt, IntoResult};

pub fn that<T: AsRef<OsStr>>(path: T) -> io::Result<()> {
    let path = path.as_ref();
    let open_handlers = [
        ("xdg-open", &[path] as &[_]),
        ("gio", &[OsStr::new("open"), path]),
        ("gnome-open", &[path]),
        ("kde-open", &[path]),
        ("wslview", &[&wsl_path(path)]),
    ];

    let mut unsuccessful = None;
    let mut io_error = None;

    for (command, args) in &open_handlers {
        let result = Command::new(command).args(*args).status_without_output();

        match result {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => {
                unsuccessful = unsuccessful.or_else(|| {
                    Some(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        status.to_string(),
                    ))
                })
            }
            Err(err) => io_error = io_error.or(Some(err)),
        }
    }

    Err(unsuccessful
        .or(io_error)
        .expect("successful cases don't get here"))
}

pub fn with<T: AsRef<OsStr>>(path: T, app: impl Into<String>) -> io::Result<()> {
    Command::new(app.into())
        .arg(path.as_ref())
        .status_without_output()
        .into_result()
}

// Polyfill to workaround absolute path bug in wslu(wslview). In versions before
// v3.1.1, wslview is unable to find absolute paths. `wsl_path` converts an
// absolute path into a relative path starting from the current directory. If
// the path is already a relative path or the conversion fails the original path
// is returned.
fn wsl_path<T: AsRef<OsStr>>(path: T) -> OsString {
    fn path_relative_to_current_dir<T: AsRef<OsStr>>(path: T) -> Option<PathBuf> {
        let path = Path::new(&path);

        if path.is_relative() {
            return None;
        }

        let base = env::current_dir().ok()?;
        pathdiff::diff_paths(path, base)
    }

    match path_relative_to_current_dir(&path) {
        None => OsString::from(&path),
        Some(relative) => OsString::from(relative),
    }
}
