// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

const GENPOLICY_ATTEMPTS: u32 = 6;
const GENPOLICY_RETRY_DELAY: Duration = Duration::from_secs(10);

/// Retry unexpected child-process outcomes, but not failures to execute the child.
pub fn run_genpolicy<C, A>(
    stdin: Option<&[u8]>,
    configure: C,
    accept: A,
) -> Result<Output, Box<dyn std::error::Error>>
where
    C: Fn(&mut Command),
    A: Fn(&Output) -> bool,
{
    retry_genpolicy(
        || {
            let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("genpolicy"));
            configure(&mut cmd);
            let mut cmd = assert_cmd::Command::from_std(cmd);
            if let Some(stdin) = stdin {
                cmd.write_stdin(stdin);
            }
            Ok(cmd.output()?)
        },
        accept,
        GENPOLICY_RETRY_DELAY,
    )
}

/// Supply the same pristine input on every attempt; only successful output is returned.
pub fn genpolicy_succeeds_stdin<C>(
    stdin: &[u8],
    configure: C,
) -> Result<Output, Box<dyn std::error::Error>>
where
    C: Fn(&mut Command),
{
    run_genpolicy(Some(stdin), configure, |output| output.status.success())
}

fn retry_genpolicy<R, A>(
    mut run: R,
    accept: A,
    delay: Duration,
) -> Result<Output, Box<dyn std::error::Error>>
where
    R: FnMut() -> Result<Output, Box<dyn std::error::Error>>,
    A: Fn(&Output) -> bool,
{
    for attempt in 1..=GENPOLICY_ATTEMPTS {
        let output = run()?;
        if accept(&output) {
            return Ok(output);
        }

        let diagnostics = format!(
            "attempt {attempt}: {}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        if attempt == GENPOLICY_ATTEMPTS {
            return Err(io::Error::other(format!(
                "genpolicy did not produce the expected outcome in {GENPOLICY_ATTEMPTS} attempts\n{diagnostics}"
            ))
            .into());
        }

        println!("retrying genpolicy after {diagnostics}");
        thread::sleep(delay);
    }

    unreachable!("the loop returns on the last attempt");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_failed_children_with_pristine_stdin() {
        let mut attempts = 0;
        let output = retry_genpolicy(
            || {
                attempts += 1;
                let mut cmd = assert_cmd::Command::new("sh");
                cmd.args(["-c", "cat; echo 'download failed' >&2; exit \"$1\"", "sh"]);
                cmd.arg(if attempts == 1 { "101" } else { "0" });
                cmd.write_stdin("pristine input");
                Ok(cmd.output()?)
            },
            |output| output.status.success(),
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(attempts, 2);
        assert_eq!(output.stdout, b"pristine input");
    }

    #[test]
    fn exhaustion_reports_last_failure() {
        let mut attempts = 0;
        let error = retry_genpolicy(
            || {
                attempts += 1;
                Ok(Command::new("sh")
                    .args([
                        "-c",
                        "printf '%s' \"$1\"; echo 'download failed' >&2; exit 101",
                        "sh",
                    ])
                    .arg(attempts.to_string())
                    .output()?)
            },
            |output| output.status.success(),
            Duration::ZERO,
        )
        .unwrap_err();

        assert_eq!(attempts, 6);
        let message = error.to_string();
        assert!(message.contains("6 attempts"), "{message}");
        assert!(message.contains("exit status: 101"), "{message}");
        assert!(message.contains("stdout: 6"), "{message}");
        assert!(message.contains("stderr: download failed"), "{message}");
    }

    #[test]
    fn execution_errors_are_not_retried() {
        let mut attempts = 0;
        let error = retry_genpolicy(
            || {
                attempts += 1;
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "cannot execute child").into())
            },
            |output| output.status.success(),
            Duration::ZERO,
        )
        .unwrap_err();

        assert_eq!(attempts, 1);
        assert!(error.to_string().contains("cannot execute child"));
    }

    #[test]
    fn negative_tests_require_expected_failure() {
        let mut attempts = 0;
        let output = retry_genpolicy(
            || {
                attempts += 1;
                let script = match attempts {
                    1 => "exit 0",
                    2 => "echo 'download failed' >&2; exit 101",
                    _ => "echo 'expected validation failure' >&2; exit 1",
                };
                Ok(Command::new("sh").args(["-c", script]).output()?)
            },
            |output| {
                !output.status.success()
                    && String::from_utf8_lossy(&output.stderr)
                        .contains("expected validation failure")
            },
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(attempts, 3);
        assert_eq!(output.status.code(), Some(1));
    }
}
