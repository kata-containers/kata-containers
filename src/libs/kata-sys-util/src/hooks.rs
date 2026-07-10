// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::{self, Result};
use std::time::Duration;

use oci_spec::runtime as oci;
use subprocess::{Exec, ExitStatus, Job, Redirection};

use crate::sl;
use crate::validate::valid_env;

const DEFAULT_HOOK_TIMEOUT_SEC: i32 = 10;

/// Mirror a hook *failure* to `/dev/kmsg`.
///
/// createContainer hooks run in the forked container child, where kata-agent's
/// async slog drain (its background thread) doesn't exist, so `error!` here is
/// silently dropped. `/dev/kmsg` is the one sink that survives the fork and
/// reaches the guest console, so without this a failed hook (e.g. a CDI hook
/// that can't be found or exits non-zero) leaves no trace at all. Failures only.
fn log_hook_failure_to_kmsg(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open("/dev/kmsg") {
        let _ = writeln!(f, "kata-agent: hook failed: {msg}");
    }
}

/// A simple wrapper over `oci::Hook` to provide `Hash, Eq`.
///
/// The `oci::Hook` is auto-generated from protobuf source file, which doesn't implement `Hash, Eq`.
#[derive(Debug, Default, Clone)]
struct HookKey(oci::Hook);

impl From<&oci::Hook> for HookKey {
    fn from(hook: &oci::Hook) -> Self {
        HookKey(hook.clone())
    }
}

impl PartialEq for HookKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for HookKey {}

impl Hash for HookKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.path().hash(state);
        self.0.args().hash(state);
        self.0.env().hash(state);
        self.0.timeout().hash(state);
    }
}

/// Execution state of OCI hooks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookState {
    /// Hook is pending for executing/retry.
    Pending,
    /// Hook has been successfully executed.
    Done,
    /// Hook has been marked as ignore.
    Ignored,
}

/// Structure to maintain state for hooks.
#[derive(Default)]
pub struct HookStates {
    states: HashMap<HookKey, HookState>,
}

impl HookStates {
    /// Create a new instance of [`HookStates`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Get execution state of a hook.
    pub fn get(&self, hook: &oci::Hook) -> HookState {
        self.states
            .get(&hook.into())
            .copied()
            .unwrap_or(HookState::Pending)
    }

    /// Update execution state of a hook.
    pub fn update(&mut self, hook: &oci::Hook, state: HookState) {
        self.states.insert(hook.into(), state);
    }

    /// Remove an execution state of a hook.
    pub fn remove(&mut self, hook: &oci::Hook) {
        self.states.remove(&hook.into());
    }

    /// Check whether some hooks are still pending and should retry execution.
    pub fn should_retry(&self) -> bool {
        for state in self.states.values() {
            if *state == HookState::Pending {
                return true;
            }
        }
        false
    }

    /// Execute an OCI hook.
    ///
    /// If `state` is valid, it will be sent to subprocess' STDIN.
    ///
    /// The [OCI Runtime specification 1.0.0](https://github.com/opencontainers/runtime-spec/releases/download/v1.0.0/oci-runtime-spec-v1.0.0.pdf)
    /// states:
    /// - path (string, REQUIRED) with similar semantics to IEEE Std 1003.1-2008 execv's path.
    ///   This specification extends the IEEE standard in that path MUST be absolute.
    /// - args (array of strings, OPTIONAL) with the same semantics as IEEE Std 1003.1-2008 execv's
    ///   argv.
    /// - env (array of strings, OPTIONAL) with the same semantics as IEEE Std 1003.1-2008's environ.
    /// - timeout (int, OPTIONAL) is the number of seconds before aborting the hook. If set, timeout
    ///   MUST be greater than zero.
    ///
    /// The OCI spec also defines the context to invoke hooks, caller needs to take the responsibility
    /// to setup execution context, such as namespace etc.
    pub fn execute_hook(
        &mut self,
        hook: &oci::Hook,
        state: Option<runtime_spec::State>,
    ) -> Result<()> {
        if self.get(hook) != HookState::Pending {
            return Ok(());
        }

        fail::fail_point!("execute_hook", |_| {
            Err(std::io::Error::other("execute hook fail point injection"))
        });
        info!(sl!(), "execute hook {:?}", hook);

        self.states.insert(hook.into(), HookState::Pending);

        let executor = HookExecutor::new(hook)?;
        let mut job = executor.spawn(state.as_ref())?;

        let communicate_result = job
            .communicate()?
            .limit_time(Duration::from_secs(executor.timeout))
            .read_string();

        match communicate_result {
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                error!(sl!(), "hook poll failed, kill it");
                let _ = job.kill();
                let _ = job.wait();
                return Err(io::Error::from(io::ErrorKind::TimedOut));
            }
            Err(e) => {
                return Err(std::io::Error::other(format!(
                    "communicate hook {hook:?}: {e}"
                )));
            }
            Ok((ref stdout, ref stderr)) => {
                if !stderr.is_empty() {
                    error!(sl!(), "hook {} stderr: {}", hook.path().display(), stderr);
                }
                if !stdout.is_empty() {
                    info!(sl!(), "hook {} stdout: {}", hook.path().display(), stdout);
                }
            }
        }

        executor.wait_and_check(&mut job)?;
        info!(sl!(), "hook {} finished", hook.path().display());
        self.states.insert(hook.into(), HookState::Done);

        Ok(())
    }

    /// Try to execute hooks and remember execution result.
    ///
    /// The `execute_hooks()` will be called multiple times.
    /// It will first be called before creating the VMM when creating the sandbox, so hooks could be
    /// used to setup environment for the VMM, such as creating tap device etc.
    /// It will also be called during starting containers, to setup environment for those containers.
    ///
    /// The execution result will be recorded for each hook. Once a hook returns success, it will not
    /// be invoked anymore.
    pub fn execute_hooks(
        &mut self,
        hooks: &[oci::Hook],
        state: Option<runtime_spec::State>,
    ) -> Result<()> {
        for hook in hooks.iter() {
            if let Err(e) = self.execute_hook(hook, state.clone()) {
                // Ignore error and try next hook, the caller should retry.
                error!(sl!(), "hook {} failed: {}", hook.path().display(), e);
                log_hook_failure_to_kmsg(&format!("{}: {e}", hook.path().display()));
            }
        }

        Ok(())
    }
}

struct HookExecutor<'a> {
    hook: &'a oci::Hook,
    executable: OsString,
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
    timeout: u64,
}

impl<'a> HookExecutor<'a> {
    fn new(hook: &'a oci::Hook) -> Result<Self> {
        // Ensure Hook.path is present and is an absolute path.
        let path = hook.path();
        if !path.exists() {
            return Err(std::io::Error::other(format!(
                "path of hook {hook:?} is empty"
            )));
        }
        if !path.is_absolute() {
            return Err(std::io::Error::other(format!(
                "path of hook {hook:?} is not absolute"
            )));
        }
        let executable = path.as_os_str().to_os_string();

        // Hook.args is optional, use Hook.path as arg0 if Hook.args is empty.
        let args: Vec<OsString> = match hook.args() {
            Some(args) => args.iter().map(OsString::from).collect(),
            None => vec![OsString::from(hook.path())],
        };

        let mut envs: Vec<(OsString, OsString)> = Vec::new();
        if let Some(env) = hook.env() {
            for e in env.iter() {
                if let Some((key, value)) = valid_env(e) {
                    envs.push((OsString::from(key), OsString::from(value)));
                }
            }
        }

        // Use Hook.timeout if it's valid, otherwise default to 10s.
        let mut timeout = DEFAULT_HOOK_TIMEOUT_SEC as u64;
        if let Some(t) = hook.timeout() {
            if t > 0 {
                timeout = t as u64;
            }
        }

        Ok(HookExecutor {
            hook,
            executable,
            args,
            envs,
            timeout,
        })
    }

    /// Spawn the subprocess, optionally feeding `state` to its stdin.
    fn spawn(&self, state: Option<&runtime_spec::State>) -> Result<Job> {
        // Execute the hook's `path` (an absolute path, validated in `new`) directly.
        // Using `args[0]` as the command would PATH-search a bare program name, which
        // only resolves when the binary lives in the compiled-in default PATH
        // (/bin:/usr/bin) after `env_clear()` - so hooks outside it (e.g. a composable
        // image extension under /run/kata-extensions/<name>/bin) fail with ENOENT.
        // OCI semantics: run `path`, with `args` as argv (argv[0] = args[0]).
        let mut exec = Exec::cmd(&self.executable)
            .args(&self.args[1..])
            .arg0(&self.args[0])
            .env_clear()
            .env_extend(
                self.envs
                    .iter()
                    .map(|(k, v)| (k.as_os_str(), v.as_os_str())),
            )
            .detached()
            .stdout(Redirection::Pipe)
            .stderr(Redirection::Pipe);

        if let Some(st) = state {
            let json = serde_json::to_string(st)?;
            exec = exec.stdin(json.into_bytes());
        } else {
            exec = exec.stdin(Redirection::None);
        }

        exec.start().map_err(|e| {
            std::io::Error::other(format!(
                "failed to create subprocess for hook {:?}: {e}",
                self.hook
            ))
        })
    }

    /// Wait for the process to finish and check its exit status.
    fn wait_and_check(&self, job: &mut Job) -> Result<()> {
        match job.wait_timeout(Duration::from_secs(1)) {
            Ok(Some(exit_status)) => {
                info!(
                    sl!(),
                    "exit status of hook {:?} : {:?}", self.hook, exit_status
                );
                self.check_exit_status(exit_status)
            }
            Ok(None) => {
                // Timeout — kill the process.
                error!(sl!(), "hook poll failed, kill it");
                let _ = job.kill();
                let _ = job.wait();
                Err(io::Error::from(io::ErrorKind::TimedOut))
            }
            Err(e) => {
                error!(sl!(), "wait_timeout for hook {:?} failed: {}", self.hook, e);
                Err(std::io::Error::other(format!(
                    "wait_timeout for hook {:?} failed: {}",
                    self.hook, e,
                )))
            }
        }
    }

    fn check_exit_status(&self, exit_status: ExitStatus) -> Result<()> {
        match exit_status.code() {
            Some(0) => {
                info!(sl!(), "hook {:?} succeeds", self.hook);
                Ok(())
            }
            Some(code) => {
                warn!(sl!(), "hook {:?} exit status with {}", self.hook, code);
                Err(std::io::Error::other(format!(
                    "hook {:?} exit status with {}",
                    self.hook, code,
                )))
            }
            None => {
                error!(
                    sl!(),
                    "no exit code for hook {:?}: {:?}", self.hook, exit_status
                );
                Err(std::io::Error::other(format!(
                    "no exit code for hook {:?}: {:?}",
                    self.hook, exit_status,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, set_permissions, File, Permissions};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Instant;

    fn test_hook_eq(hook1: &oci::Hook, hook2: &oci::Hook, expected: bool) {
        let key1 = HookKey::from(hook1);
        let key2 = HookKey::from(hook2);

        assert_eq!(key1 == key2, expected);
    }

    // struct Hook is just for test cases
    #[derive(Clone)]
    pub struct Hook {
        pub path: String,
        pub args: Vec<String>,
        pub env: Vec<String>,
        pub timeout: Option<i64>,
    }

    impl Hook {
        fn build_oci_hook(self) -> oci::Hook {
            let mut hook = oci::Hook::default();
            hook.set_path(PathBuf::from(self.path));
            if self.args.is_empty() {
                hook.set_args(None);
            } else {
                hook.set_args(Some(self.args));
            }
            hook.set_env(Some(self.env));
            hook.set_timeout(self.timeout);

            hook
        }
    }

    #[test]
    fn test_hook_key() {
        let hook = Hook {
            path: "1".to_string(),
            args: vec!["2".to_string(), "3".to_string()],
            env: vec![],
            timeout: Some(0),
        };
        let oci_hook = hook.build_oci_hook();

        let cases = [
            (
                Hook {
                    path: "1000".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "4".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec!["5".to_string()],
                    timeout: Some(0),
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(6),
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: None,
                },
                false,
            ),
            (
                Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                true,
            ),
        ];

        for case in cases.iter() {
            let case_hook = case.0.clone().build_oci_hook();
            test_hook_eq(&oci_hook, &case_hook, case.1);
        }
    }

    #[test]
    fn test_execute_hook() {
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }

        let tmpdir = tempfile::tempdir().unwrap();
        let file = tmpdir.path().join("data");
        let file_str = file.to_string_lossy();
        let mut states = HookStates::new();

        // test case 1: normal
        // execute hook
        let hook = Hook {
            path: "/bin/touch".to_string(),
            args: vec!["touch".to_string(), file_str.to_string()],
            env: vec![],
            timeout: Some(0),
        };
        let oci_hook = hook.build_oci_hook();
        let ret = states.execute_hook(&oci_hook, None);
        assert!(ret.is_ok());
        assert!(fs::metadata(&file).is_ok());
        assert!(!states.should_retry());

        // test case 2: timeout in 10s
        let hook = Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(0), // default timeout is 10 seconds
        };
        let oci_hook = hook.build_oci_hook();
        let start = Instant::now();
        let ret = states.execute_hook(&oci_hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((10..12u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert_eq!(states.get(&oci_hook), HookState::Pending);
        assert!(states.should_retry());
        states.remove(&oci_hook);

        // test case 3: timeout in 5s
        let hook = Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(5), // timeout is set to 5 seconds
        };
        let oci_hook = hook.build_oci_hook();
        let start = Instant::now();
        let ret = states.execute_hook(&oci_hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((5..7u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert_eq!(states.get(&oci_hook), HookState::Pending);
        assert!(states.should_retry());
        states.remove(&oci_hook);

        // test case 4: with envs
        let create_shell = |shell_path: &str, data_path: &str| -> Result<()> {
            let shell = format!(
                r#"#!/bin/sh
echo -n "K1=${{K1}}" > {data_path}
"#
            );
            let mut output = File::create(shell_path)?;
            output.write_all(shell.as_bytes())?;

            // set to executable
            let permissions = Permissions::from_mode(0o755);
            set_permissions(shell_path, permissions)?;

            Ok(())
        };
        let shell_path = format!("{}/test.sh", tmpdir.path().to_string_lossy());
        let ret = create_shell(&shell_path, file_str.as_ref());
        assert!(ret.is_ok());
        let hook = Hook {
            path: shell_path,
            args: vec![],
            env: vec!["K1=V1".to_string()],
            timeout: Some(5),
        };
        let oci_hook = hook.build_oci_hook();
        let ret = states.execute_hook(&oci_hook, None);
        assert!(ret.is_ok());
        assert!(!states.should_retry());
        let contents = fs::read_to_string(file);
        match contents {
            Err(e) => panic!("got error {}", e),
            Ok(s) => assert_eq!(s, "K1=V1"),
        }

        // test case 5: timeout in 5s with state
        let hook = Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(6), // timeout is set to 5 seconds
        };
        let oci_hook = hook.build_oci_hook();
        let state = runtime_spec::State {
            version: "".to_string(),
            id: "".to_string(),
            status: runtime_spec::ContainerState::Creating,
            pid: 10,
            bundle: "nouse".to_string(),
            annotations: Default::default(),
        };
        let start = Instant::now();
        let ret = states.execute_hook(&oci_hook, Some(state)).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((6..8u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert!(states.should_retry());
    }
}
