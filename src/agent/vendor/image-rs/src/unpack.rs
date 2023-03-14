// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use libc::timeval;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::io;
use std::path::Path;
use tar::Archive;

/// Unpack the contents of tarball to the destination path
pub fn unpack<R: io::Read>(input: R, destination: &Path) -> Result<()> {
    let mut archive = Archive::new(input);

    if destination.exists() {
        bail!("unpack destination {:?} already exists", destination);
    }

    fs::create_dir_all(destination)?;

    let mut dirs: HashMap<CString, [timeval; 2]> = HashMap::default();
    for file in archive.entries()? {
        let mut file = file?;
        file.unpack_in(destination)?;

        // tar-rs crate only preserve timestamps of files,
        // symlink file and directory are not covered.
        // upstream fix PR: https://github.com/alexcrichton/tar-rs/pull/217
        if file.header().entry_type().is_symlink() || file.header().entry_type().is_dir() {
            let mtime = file.header().mtime()? as i64;

            let atime = timeval {
                tv_sec: mtime,
                tv_usec: 0,
            };
            let path = CString::new(format!(
                "{}/{}",
                destination.display(),
                file.path()?.display()
            ))?;

            let times = [atime, atime];

            if file.header().entry_type().is_dir() {
                dirs.insert(path, times);
            } else {
                let ret = unsafe { libc::lutimes(path.as_ptr(), times.as_ptr()) };
                if ret != 0 {
                    bail!(
                        "change symlink file: {:?} utime error: {:?}",
                        path,
                        io::Error::last_os_error()
                    );
                }
            }
        }
    }

    // Directory timestamps need update after all files are extracted.
    for (k, v) in dirs.iter() {
        let ret = unsafe { libc::utimes(k.as_ptr(), v.as_ptr()) };
        if ret != 0 {
            bail!(
                "change directory: {:?} utime error: {:?}",
                k,
                io::Error::last_os_error()
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime;
    use std::fs::File;
    use std::io::prelude::*;
    use tempfile;

    #[test]
    fn test_unpack() {
        let mut ar = tar::Builder::new(Vec::new());
        let tempdir = tempfile::tempdir().unwrap();

        let path = tempdir.path().join("file.txt");
        File::create(&path)
            .unwrap()
            .write_all(b"file data")
            .unwrap();

        let mtime = filetime::FileTime::from_unix_time(20_000, 0);
        filetime::set_file_mtime(&path, mtime).unwrap();
        ar.append_file("file.txt", &mut File::open(&path).unwrap())
            .unwrap();

        let path = tempdir.path().join("dir");
        fs::create_dir(&path).unwrap();

        filetime::set_file_mtime(&path, mtime).unwrap();
        ar.append_path_with_name(&path, "dir").unwrap();

        // TODO: Add more file types like symlink, char, block devices.
        let data = ar.into_inner().unwrap();
        tempdir.close().unwrap();

        let destination = Path::new("/tmp/image_test_dir");
        if destination.exists() {
            fs::remove_dir_all(destination).unwrap();
        }

        assert!(unpack(data.as_slice(), destination).is_ok());

        let path = destination.join("file.txt");
        let metadata = fs::metadata(&path).unwrap();
        let new_mtime = filetime::FileTime::from_last_modification_time(&metadata);
        assert_eq!(mtime, new_mtime);

        let path = destination.join("dir");
        let metadata = fs::metadata(&path).unwrap();
        let new_mtime = filetime::FileTime::from_last_modification_time(&metadata);
        assert_eq!(mtime, new_mtime);

        // destination already exists
        assert!(unpack(data.as_slice(), destination).is_err());
    }
}
