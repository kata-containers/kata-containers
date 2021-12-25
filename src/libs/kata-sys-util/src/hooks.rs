// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Result};
use std::path::Path;
use std::time::Duration;

use subprocess::{ExitStatus, Popen, PopenConfig, PopenError, Redirection};

use crate::{eother, sl};

const DEFAULT_HOOK_TIMEOUT_SEC: i32 = 10;

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
        self.0.path.hash(state);
        self.0.args.hash(state);
        self.0.env.hash(state);
        self.0.timeout.hash(state);
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
    pub fn execute_hook(&mut self, hook: &oci::Hook, state: Option<oci::State>) -> Result<()> {
        if self.get(hook) != HookState::Pending {
            return Ok(());
        }

        fail::fail_point!("execute_hook", |_| {
            Err(eother!("execute hook fail point injection"))
        });
        info!(sl!(), "execute hook {:?}", hook);

        self.states.insert(hook.into(), HookState::Pending);

        let mut executor = HookExecutor::new(hook)?;
        let stdin = if state.is_some() {
            Redirection::Pipe
        } else {
            Redirection::None
        };
        let mut popen = Popen::create(
            &executor.args,
            PopenConfig {
                stdin,
                stdout: Redirection::Pipe,
                stderr: Redirection::Pipe,
                executable: executor.executable.to_owned(),
                detached: true,
                env: Some(executor.envs.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| eother!("failed to create subprocess for hook {:?}: {}", hook, e))?;

        if let Some(state) = state {
            executor.execute_with_input(&mut popen, state)?;
        }
        executor.execute_and_wait(&mut popen)?;
        info!(sl!(), "hook {} finished", hook.path);
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
    pub fn execute_hooks(&mut self, hooks: &[oci::Hook], state: Option<oci::State>) -> Result<()> {
        for hook in hooks.iter() {
            if let Err(e) = self.execute_hook(hook, state.clone()) {
                // Ignore error and try next hook, the caller should retry.
                error!(sl!(), "hook {} failed: {}", hook.path, e);
            }
        }

        Ok(())
    }
}

struct HookExecutor<'a> {
    hook: &'a oci::Hook,
    executable: Option<OsString>,
    args: Vec<String>,
    envs: Vec<(OsString, OsString)>,
    timeout: u64,
}

impl<'a> HookExecutor<'a> {
    fn new(hook: &'a oci::Hook) -> Result<Self> {
        // Ensure Hook.path is present and is an absolute path.
        let executable = if hook.path.is_empty() {
            return Err(eother!("path of hook {:?} is empty", hook));
        } else {
            let path = Path::new(&hook.path);
            if !path.is_absolute() {
                return Err(eother!("path of hook {:?} is not absolute", hook));
            }
            Some(path.as_os_str().to_os_string())
        };

        // Hook.args is optional, use Hook.path as arg0 if Hook.args is empty.
        let args = if hook.args.is_empty() {
            vec![hook.path.clone()]
        } else {
            hook.args.clone()
        };

        let mut envs: Vec<(OsString, OsString)> = Vec::new();
        for e in hook.env.iter() {
            match e.split_once('=') {
                Some((key, value)) => envs.push((OsString::from(key), OsString::from(value))),
                None => warn!(sl!(), "env {} of hook {:?} is invalid", e, hook),
            }
        }

        // Use Hook.timeout if it's valid, otherwise default to 10s.
        let mut timeout = DEFAULT_HOOK_TIMEOUT_SEC as u64;
        if let Some(t) = hook.timeout {
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

    fn execute_with_input(&mut self, popen: &mut Popen, state: oci::State) -> Result<()> {
        let st = serde_json::to_string(&state)?;
        let (stdout, stderr) = popen
            .communicate_start(Some(st.as_bytes().to_vec()))
            .limit_time(Duration::from_secs(self.timeout))
            .read_string()
            .map_err(|e| e.error)?;
        if let Some(err) = stderr {
            if !err.is_empty() {
                error!(sl!(), "hook {} exec failed: {}", self.hook.path, err);
            }
        }
        if let Some(out) = stdout {
            if !out.is_empty() {
                info!(sl!(), "hook {} exec stdout: {}", self.hook.path, out);
            }
        }
        // Give a grace period for `execute_and_wait()`.
        self.timeout = 1;
        Ok(())
    }

    fn execute_and_wait(&mut self, popen: &mut Popen) -> Result<()> {
        match popen.wait_timeout(Duration::from_secs(self.timeout)) {
            Ok(v) => self.handle_exit_status(v, popen),
            Err(e) => self.handle_popen_wait_error(e, popen),
        }
    }

    fn handle_exit_status(&mut self, result: Option<ExitStatus>, popen: &mut Popen) -> Result<()> {
        if let Some(exit_status) = result {
            // the process has finished
            info!(
                sl!(),
                "exit status of hook {:?} : {:?}", self.hook, exit_status
            );
            self.print_result(popen);
            match exit_status {
                subprocess::ExitStatus::Exited(code) => {
                    if code == 0 {
                        info!(sl!(), "hook {:?} succeeds", self.hook);
                        Ok(())
                    } else {
                        warn!(sl!(), "hook {:?} exit status with {}", self.hook, code,);
                        Err(eother!("hook {:?} exit status with {}", self.hook, code))
                    }
                }
                _ => {
                    error!(
                        sl!(),
                        "no exit code for hook {:?}: {:?}", self.hook, exit_status
                    );
                    Err(eother!(
                        "no exit code for hook {:?}: {:?}",
                        self.hook,
                        exit_status
                    ))
                }
            }
        } else {
            // may be timeout
            error!(sl!(), "hook poll failed, kill it");
            // it is still running, kill it
            popen.kill()?;
            let _ = popen.wait();
            self.print_result(popen);
            Err(io::Error::from(io::ErrorKind::TimedOut))
        }
    }

    fn handle_popen_wait_error(&mut self, e: PopenError, popen: &mut Popen) -> Result<()> {
        self.print_result(popen);
        error!(sl!(), "wait_timeout for hook {:?} failed: {}", self.hook, e);
        Err(eother!(
            "wait_timeout for hook {:?} failed: {}",
            self.hook,
            e
        ))
    }

    fn print_result(&mut self, popen: &mut Popen) {
        if let Some(file) = popen.stdout.as_mut() {
            let mut buffer = String::new();
            file.read_to_string(&mut buffer).ok();
            if !buffer.is_empty() {
                info!(sl!(), "hook stdout: {}", buffer);
            }
        }
        if let Some(file) = popen.stderr.as_mut() {
            let mut buffer = String::new();
            file.read_to_string(&mut buffer).ok();
            if !buffer.is_empty() {
                info!(sl!(), "hook stderr: {}", buffer);
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
    use std::time::Instant;

    fn test_hook_eq(hook1: &oci::Hook, hook2: &oci::Hook, expected: bool) {
        let key1 = HookKey::from(hook1);
        let key2 = HookKey::from(hook2);

        assert_eq!(key1 == key2, expected);
    }
    #[test]
    fn test_hook_key() {
        let hook = oci::Hook {
            path: "1".to_string(),
            args: vec!["2".to_string(), "3".to_string()],
            env: vec![],
            timeout: Some(0),
        };
        let cases = [
            (
                oci::Hook {
                    path: "1000".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "4".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec!["5".to_string()],
                    timeout: Some(0),
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(6),
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: None,
                },
                false,
            ),
            (
                oci::Hook {
                    path: "1".to_string(),
                    args: vec!["2".to_string(), "3".to_string()],
                    env: vec![],
                    timeout: Some(0),
                },
                true,
            ),
        ];

        for case in cases.iter() {
            test_hook_eq(&hook, &case.0, case.1);
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
        let hook = oci::Hook {
            path: "/bin/touch".to_string(),
            args: vec!["touch".to_string(), file_str.to_string()],
            env: vec![],
            timeout: Some(0),
        };
        let ret = states.execute_hook(&hook, None);
        assert!(ret.is_ok());
        assert!(fs::metadata(&file).is_ok());
        assert!(!states.should_retry());

        // test case 2: timeout in 10s
        let hook = oci::Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(0), // default timeout is 10 seconds
        };
        let start = Instant::now();
        let ret = states.execute_hook(&hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((10..12u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert_eq!(states.get(&hook), HookState::Pending);
        assert!(states.should_retry());
        states.remove(&hook);

        // test case 3: timeout in 5s
        let hook = oci::Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(5), // timeout is set to 5 seconds
        };
        let start = Instant::now();
        let ret = states.execute_hook(&hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((5..7u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert_eq!(states.get(&hook), HookState::Pending);
        assert!(states.should_retry());
        states.remove(&hook);

        // test case 4: with envs
        let create_shell = |shell_path: &str, data_path: &str| -> Result<()> {
            let shell = format!(
                r#"#!/bin/sh
echo -n "K1=${{K1}}" > {}
"#,
                data_path
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
        let hook = oci::Hook {
            path: shell_path,
            args: vec![],
            env: vec!["K1=V1".to_string()],
            timeout: Some(5),
        };
        let ret = states.execute_hook(&hook, None);
        assert!(ret.is_ok());
        assert!(!states.should_retry());
        let contents = fs::read_to_string(file);
        match contents {
            Err(e) => panic!("got error {}", e),
            Ok(s) => assert_eq!(s, "K1=V1"),
        }

        // test case 5: timeout in 5s with state
        let hook = oci::Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(6), // timeout is set to 5 seconds
        };
        let state = oci::State {
            version: "".to_string(),
            id: "".to_string(),
            status: oci::ContainerState::Creating,
            pid: 10,
            bundle: "nouse".to_string(),
            annotations: Default::default(),
        };
        let start = Instant::now();
        let ret = states.execute_hook(&hook, Some(state)).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((6..8u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
        assert!(states.should_retry());
    }
}
