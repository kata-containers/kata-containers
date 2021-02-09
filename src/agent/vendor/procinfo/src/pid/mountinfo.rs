//! Information about mounts from `/proc/[pid]/mountinfo`.

use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::io::{Error, ErrorKind};

use libc::pid_t;
use nom::{Err, IResult, Needed};
use nom::ErrorKind::Tag;

use parsers::{map_result, parse_isize, parse_usize};

/// Process mounts information.
///
/// See `proc(5)` for format details.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Mountinfo {
    /// Unique ID for the mount.
    pub mount_id: isize,
    /// ID of the parent mount.
    pub parent_id: isize,
    /// Device major ID (class).
    pub major: usize,
    /// Device minor ID (instance).
    pub minor: usize,
    /// Pathname which forms the root of this mount.
    pub root: PathBuf,
    /// Mount pathname relative to the process's root.
    pub mount_point: PathBuf,
    /// mount options.
    pub mount_options: Vec<MountOption>,
    /// Optional fields (tag with optional value).
    pub opt_fields: Vec<OptionalField>,
    /// Filesystem type (main type with optional sub-type).
    pub fs_type: (String, Option<String>),
    /// Filesystem specific information.
    pub mount_src: Option<String>,
    /// Superblock options.
    pub super_opts: Vec<String>,
}

/// Mountinfo optional field
///
/// See `proc(5)` and `mount_namespace(7)` for more details.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum OptionalField {
    /// A mount shared in peer group `ID`
    Shared(usize),
    /// A mount which is a slave of shared peer group `ID`
    Master(usize),
    /// A slave mount which receives propagation events from
    /// shared peer group `ID`
    PropagateFrom(usize),
    /// An unbindable mount
    Unbindable,
    /// A private mount
    Private
}

/// Mountpoint option
///
/// See `mount(8)` for more details.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum MountOption {
    /// Do not update inode access time
    Noatime,
    /// Do not interpret special device files
    Nodev,
    /// Do not update directory inode access time
    Nodiratime,
    /// No direct binary execution
    Noexec,
    /// Do not allow suid and sgid bits effects
    Nosuid,
    /// Conditionally update inode access time
    Relatime,
    /// Read-only
    Ro,
    /// Read-write
    Rw,
    /// Other custom options
    Other(String),
}

/// Consumes a space, main fields separator and optional fields separator
named!(space, tag!(" "));

/// Consumes an hypen, the optional fields terminator
named!(hypen, tag!("-"));

/// Consumes a colon, the major-minor separator
named!(colon, tag!(":"));

/// Consumes a dot, the fs sub-type separator
named!(dot, tag!("."));

/// Parses a space-terminated string field in a mountinfo entry
named!(parse_string_field<String>,
       map_res!(map_res!(is_not!(" "), str::from_utf8), FromStr::from_str));


/// Parses a string of optional fields.
fn mount_options(opts: String) -> Vec<MountOption> {
    opts.split(",").map(|o|
        match o {
            "noatime"    => MountOption::Noatime,
            "nodev"      => MountOption::Nodev,
            "nodiratime" => MountOption::Nodiratime,
            "noexec"     => MountOption::Noexec,
            "nosuid"     => MountOption::Nosuid,
            "relatime"   => MountOption::Relatime,
            "ro"         => MountOption::Ro,
            "rw"         => MountOption::Rw,
            x            => MountOption::Other(x.into()),
        }
    ).collect()
}

/// Parses a comma-separated list of mount options.
named!(parse_mnt_options<Vec<MountOption> >,
       do_parse!(token: parse_string_field >>
                 (mount_options(token))
       )
);

/// Parses a string of optional fields.
fn opt_fields(fs: &str) -> Result<Vec<OptionalField>> {
    let mut v = Vec::new();

    for i in fs.split_terminator(' ') {
        let t: Vec<&str> = i.split(':').collect();
        if t.len() > 2 {
            return Err(Error::new(ErrorKind::InvalidInput, "too many colons"));
        }
        match (t.get(0), t.get(1)) {
            (Some(&"shared"), Some(x)) if usize::from_str(x).is_ok() =>
                v.push(OptionalField::Shared(usize::from_str(x).unwrap())),
            (Some(&"master"), Some(x)) if usize::from_str(x).is_ok() =>
                v.push(OptionalField::Master(usize::from_str(x).unwrap())),
            (Some(&"propagate_from"), Some(x)) if usize::from_str(x).is_ok() =>
                v.push(OptionalField::PropagateFrom(usize::from_str(x).unwrap())),
            (Some(&"unbindable"), None) =>
                v.push(OptionalField::Unbindable),
            (_, _) => return Err(Error::new(ErrorKind::InvalidInput, "invalid optional value")),
        };
    }

    if v.len() == 0 {
        v.push(OptionalField::Private);
    }

    return Ok(v);
}

/// Parses a space-separated list of tag:value optional fields.
fn parse_opt_fields(input: &[u8]) -> IResult<&[u8], Vec<OptionalField>> {
    // look for the mandatory terminator (hypen)
    let mut hypen = None;
    for idx in 0..input.len() {
        if '-' as u8 == input[idx] {
            hypen = Some(idx);
            break
        }
    }
    if hypen.is_none() {
        return IResult::Incomplete(Needed::Unknown);
    }

    // parse all optional fields
    let term = hypen.unwrap();
    let fs = str::from_utf8(&input[0..term]);
    match fs {
        Err(_) => IResult::Error(Err::Position(Tag, input)),
        Ok(f) => match opt_fields(f) {
            Err(_) => IResult::Error(Err::Position(Tag, input)),
            Ok(r) => IResult::Done(&input[term..], r),
        }
    }
}

/// Parses a fs type label, with optional dotted sub-type.
named!(parse_fs_type<(String, Option<String>)>,
       do_parse!(k: map_res!(map_res!(take_until_either!(" ."), str::from_utf8), FromStr::from_str) >>
                 v: opt!(do_parse!(dot >> s: parse_string_field >> (s))) >>
                 (k, v)
       )
);

/// Parses a mount source.
named!(parse_mount_src<Option<String> >,
       do_parse!(src: parse_string_field >>
                 (if src == "none" { None } else { Some(src) })
       )
);

/// Parses a comma-separated list of options.
named!(parse_options<Vec<String> >,
       do_parse!(token: parse_string_field >>
                 (token.split(",").map(|s| s.into()).collect())
       )
);

/// Parses a mountpoint entry according to mountinfo file format.
named!(parse_mountinfo_entry<Mountinfo>,
    do_parse!(mount_id: parse_isize            >> space >>
              parent_id: parse_isize           >> space >>
              major: parse_usize               >> colon >>
              minor: parse_usize               >> space >>
              root: parse_string_field         >> space >>
              mount_point: parse_string_field  >> space >>
              mount_options: parse_mnt_options >> space >>
              opt_fields: parse_opt_fields     >> hypen >> space >>
              fs_type: parse_fs_type           >> space >>
              mount_src: parse_mount_src       >> space >>
              super_opts: parse_options        >>
              ( Mountinfo {
                            mount_id: mount_id,
                            parent_id: parent_id,
                            major: major,
                            minor: minor,
                            root: root.into(),
                            mount_point: mount_point.into(),
                            mount_options: mount_options,
                            opt_fields: opt_fields,
                            fs_type: fs_type,
                            mount_src: mount_src,
                            super_opts: super_opts,
           } )));

/// Parses the provided mountinfo file.
fn mountinfo_file(file: &mut File) -> Result<Vec<Mountinfo>> {
    let mut r = Vec::new();
    for line in BufReader::new(file).lines() {
        let mi = try!(map_result(parse_mountinfo_entry(try!(line).as_bytes())));
        r.push(mi);
    }
    return Ok(r);
}

/// Returns mounts information for the process with the provided pid.
pub fn mountinfo(pid: pid_t) -> Result<Vec<Mountinfo>> {
    mountinfo_file(&mut try!(File::open(&format!("/proc/{}/mountinfo", pid))))
}

/// Returns mounts information for the current process.
pub fn mountinfo_self() -> Result<Vec<Mountinfo>> {
    mountinfo_file(&mut try!(File::open("/proc/self/mountinfo")))
}

#[cfg(test)]
pub mod tests {
    use super::{Mountinfo, MountOption, OptionalField, mountinfo, mountinfo_self, parse_mountinfo_entry};

    /// Test parsing a single mountinfo entry (positive check).
    #[test]
    fn test_parse_mountinfo_entry() {
        let entry =
            b"19 23 0:4 / /proc rw,nosuid,foo shared:13 master:20 - proc.sys proc rw,nosuid";
        let got_mi = parse_mountinfo_entry(entry).unwrap().1;
        let want_mi = Mountinfo {
            mount_id: 19,
            parent_id: 23,
            major: 0,
            minor: 4,
            root: "/".into(),
            mount_point: "/proc".into(),
            mount_options: vec![
                MountOption::Rw,
                MountOption::Nosuid,
                MountOption::Other("foo".to_string())
            ],
            opt_fields: vec![
                OptionalField::Shared(13),
                OptionalField::Master(20)
            ],
            fs_type: ("proc".to_string(), Some("sys".to_string())),
            mount_src: Some("proc".to_string()),
            super_opts: vec!["rw","nosuid"].iter().map(|&s| s.into()).collect(),
        };
        assert_eq!(got_mi, want_mi);
    }

    /// Test parsing a single mountinfo entry (negative check).
    #[test]
    fn test_parse_mountinfo_error() {
        let entry = b"10 - 0:4 / /sys rw master -";
        parse_mountinfo_entry(entry).unwrap_err();
    }

    /// Test that the system mountinfo files can be parsed.
    #[test]
    fn test_mountinfo() {
        mountinfo_self().unwrap();
        mountinfo(1).unwrap();
    }
}
