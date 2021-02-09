// For the 'try!' macro, until we bump MSRV past 1.12.
#![allow(deprecated)]

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::Command;

fn main() {
    let version = match Version::read() {
        Ok(version) => version,
        Err(err) => {
            writeln!(
                &mut io::stderr(),
                "failed to parse `rustc --version`: {}",
                err
            ).unwrap();
            return;
        }
    };
    enable_i128(version);
}

fn enable_i128(version: Version) {
    if version < (Version { major: 1, minor: 26, patch: 0 }) {
        return;
    }

    println!("cargo:rustc-cfg=byteorder_i128");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    fn read() -> Result<Version, String> {
        let rustc = env::var_os("RUSTC").unwrap_or(OsString::from("rustc"));
        let output = Command::new(&rustc)
            .arg("--version")
            .output()
            .unwrap()
            .stdout;
        Version::parse(&String::from_utf8(output).unwrap())
    }

    fn parse(mut s: &str) -> Result<Version, String> {
        if !s.starts_with("rustc ") {
            return Err(format!("unrecognized version string: {}", s));
        }
        s = &s["rustc ".len()..];

        let parts: Vec<&str> = s.split(".").collect();
        if parts.len() < 3 {
            return Err(format!("not enough version parts: {:?}", parts));
        }

        let mut num = String::new();
        for c in parts[0].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let major = try!(num.parse::<u32>().map_err(|e| e.to_string()));

        num.clear();
        for c in parts[1].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let minor = try!(num.parse::<u32>().map_err(|e| e.to_string()));

        num.clear();
        for c in parts[2].chars() {
            if !c.is_digit(10) {
                break;
            }
            num.push(c);
        }
        let patch = try!(num.parse::<u32>().map_err(|e| e.to_string()));

        Ok(Version { major: major, minor: minor, patch: patch })
    }
}
