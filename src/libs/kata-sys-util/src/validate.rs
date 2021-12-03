// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid container ID {0}")]
    InvalidContainerID(String),
}

// A container ID or exec ID must match this regex:
//
//     ^[a-zA-Z0-9][a-zA-Z0-9_.-]+$
//
pub fn verify_id(id: &str) -> Result<(), Error> {
    let mut chars = id.chars();

    let valid = match chars.next() {
        Some(first)
            if first.is_alphanumeric()
                && id.len() > 1
                && chars.all(|c| c.is_alphanumeric() || ['.', '-', '_'].contains(&c)) =>
        {
            true
        }
        _ => false,
    };

    match valid {
        true => Ok(()),
        false => Err(Error::InvalidContainerID(id.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_cid() {
        #[derive(Debug)]
        struct TestData<'a> {
            id: &'a str,
            expect_error: bool,
        }

        let tests = &[
            TestData {
                // Cannot be blank
                id: "",
                expect_error: true,
            },
            TestData {
                // Cannot be a space
                id: " ",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: ".",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "_",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: " a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: ".a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "_a",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "..",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "a",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "z",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "A",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "Z",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "0",
                expect_error: true,
            },
            TestData {
                // Too short
                id: "9",
                expect_error: true,
            },
            TestData {
                // Must start with an alphanumeric
                id: "-1",
                expect_error: true,
            },
            TestData {
                id: "/",
                expect_error: true,
            },
            TestData {
                id: "a/",
                expect_error: true,
            },
            TestData {
                id: "a/../",
                expect_error: true,
            },
            TestData {
                id: "../a",
                expect_error: true,
            },
            TestData {
                id: "../../a",
                expect_error: true,
            },
            TestData {
                id: "../../../a",
                expect_error: true,
            },
            TestData {
                id: "foo/../bar",
                expect_error: true,
            },
            TestData {
                id: "foo bar",
                expect_error: true,
            },
            TestData {
                id: "a.",
                expect_error: false,
            },
            TestData {
                id: "a..",
                expect_error: false,
            },
            TestData {
                id: "aa",
                expect_error: false,
            },
            TestData {
                id: "aa.",
                expect_error: false,
            },
            TestData {
                id: "hello..world",
                expect_error: false,
            },
            TestData {
                id: "hello/../world",
                expect_error: true,
            },
            TestData {
                id: "aa1245124sadfasdfgasdga.",
                expect_error: false,
            },
            TestData {
                id: "aAzZ0123456789_.-",
                expect_error: false,
            },
            TestData {
                id: "abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: false,
            },
            TestData {
                id: "0123456789abcdefghijklmnopqrstuvwxyz.-_",
                expect_error: false,
            },
            TestData {
                id: " abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: ".abcdefghijklmnopqrstuvwxyz0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: false,
            },
            TestData {
                id: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ.-_",
                expect_error: false,
            },
            TestData {
                id: " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: ".ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_",
                expect_error: true,
            },
            TestData {
                id: "/a/b/c",
                expect_error: true,
            },
            TestData {
                id: "a/b/c",
                expect_error: true,
            },
            TestData {
                id: "foo/../../../etc/passwd",
                expect_error: true,
            },
            TestData {
                id: "../../../../../../etc/motd",
                expect_error: true,
            },
            TestData {
                id: "/etc/passwd",
                expect_error: true,
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = verify_id(d.id);

            let msg = format!("{}, result: {:?}", msg, result);

            if result.is_ok() {
                assert!(!d.expect_error, "{}", msg);
            } else {
                assert!(d.expect_error, "{}", msg);
            }
        }
    }
}
