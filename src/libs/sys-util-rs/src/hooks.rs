// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lazy_static::lazy_static;
use subprocess::{Popen, PopenConfig, Redirection};

use crate::{eother, sl};

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

lazy_static! {
    static ref EXEC_RESULT: Arc<Mutex<HashMap<u64, HookState>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

fn calculate_hash(hook: &oci::Hook) -> u64 {
    let mut t = hook.path.clone();
    for a in &hook.args {
        t.push_str(a);
    }
    for a in &hook.env {
        t.push_str(a);
    }
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

/// Get execution state of a hook.
pub fn get_hook_state(hook: &oci::Hook) -> HookState {
    let key = calculate_hash(hook);
    EXEC_RESULT
        .lock()
        .unwrap()
        .get(&key)
        .copied()
        .unwrap_or(HookState::Pending)
}

/// Update execution state of a hook.
pub fn update_hook_state(hook: &oci::Hook, state: HookState) {
    let key = calculate_hash(hook);
    EXEC_RESULT.lock().unwrap().insert(key, state);
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
pub fn execute_hooks(hooks: &[oci::Hook], state: Option<oci::State>) -> Result<()> {
    for hook in hooks.iter() {
        let key = calculate_hash(hook);
        if let Some(HookState::Pending) = EXEC_RESULT.lock().unwrap().get(&key) {
            match execute_hook(hook, state.clone()) {
                Err(e) => {
                    error!(sl!(), "hook {} failed: {}", hook.path, e);
                    // Cannot return err, otherwise the subsequent hook will not be executed
                    EXEC_RESULT.lock().unwrap().insert(key, HookState::Pending);
                }
                Ok(()) => {
                    info!(sl!(), "hook {} finished", hook.path);
                    EXEC_RESULT.lock().unwrap().insert(key, HookState::Done);
                }
            }
        }
    }

    Ok(())
}

/// Execute an OCI hook.
///
/// If `state` is valid, it will be sent to subprocess' STDIN.
///
/// The OCI Runtime specification states:
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
pub fn execute_hook(hook: &oci::Hook, state: Option<oci::State>) -> Result<()> {
    fail::fail_point!("execute_hook", |_| {
        Err(eother!("execute hook fail point injection"))
    });
    info!(sl!(), "execute hook {:?}", hook);

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
        let v: Vec<&str> = e.splitn(2, '=').collect();
        if v.len() == 2 {
            envs.push((OsString::from(v[0]), OsString::from(v[1])));
        } else {
            warn!(sl!(), "env {} of hook {:?} is invalid", e, hook);
        }
    }

    // Use Hook.timeout if it's valid, otherwise default to 10s.
    let mut timeout: u64 = 10;
    if let Some(t) = hook.timeout {
        if t > 0 {
            timeout = t as u64;
        }
    }

    let stdin = if state.is_some() {
        Redirection::Pipe
    } else {
        Redirection::None
    };

    let mut popen = Popen::create(
        &args,
        PopenConfig {
            stdin,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            executable,
            detached: true,
            env: Some(envs),
            ..Default::default()
        },
    )
    .map_err(|e| eother!("failed to create subprocess for hook {:?}: {}", hook, e))?;

    if let Some(state) = state {
        let st = serde_json::to_string(&state)?;
        let (stdout, stderr) = popen
            .communicate_start(Some(st.as_bytes().to_vec()))
            .limit_time(Duration::from_secs(timeout))
            .read_string()
            .map_err(|e| e.error)?;
        if let Some(err) = stderr {
            if !err.is_empty() {
                error!(sl!(), "hook {} exec failed: {}", hook.path, err);
            }
        }
        if let Some(out) = stdout {
            if !out.is_empty() {
                info!(sl!(), "hook {} exec stdout: {}", hook.path, out);
            }
        }
        timeout = 1;
    }

    info!(sl!(), "wait with timeout {}", timeout);
    let print_result = |popen: &mut Popen| {
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
    };
    match popen.wait_timeout(Duration::from_secs(timeout)) {
        Ok(v) => {
            if let Some(exit_status) = v {
                // the process has finished
                info!(sl!(), "exit status of hook {:?} : {:?}", hook, exit_status);
                print_result(&mut popen);
                match exit_status {
                    subprocess::ExitStatus::Exited(code) => {
                        if code == 0 {
                            info!(sl!(), "hook {:?} succeeds", hook);
                            Ok(())
                        } else {
                            warn!(sl!(), "hook {:?} exit status with {}", hook, code,);
                            Err(eother!("hook {:?} exit status with {}", hook, code))
                        }
                    }
                    _ => {
                        error!(sl!(), "no exit code for hook {:?}: {:?}", hook, exit_status);
                        Err(eother!(
                            "no exit code for hook {:?}: {:?}",
                            hook,
                            exit_status
                        ))
                    }
                }
            } else {
                // may be timeout
                error!(sl!(), "hook poll failed, kill it");
                // it is still running, kill it
                popen.kill()?;
                // TODO: should wait?
                // p.wait()?;
                print_result(&mut popen);
                Err(io::Error::from(io::ErrorKind::TimedOut))
            }
        }
        Err(e) => {
            print_result(&mut popen);
            error!(sl!(), "wait_timeout for hook {:?} failed: {}", hook, e);
            Err(eother!("wait_timeout for hook {:?} failed: {}", hook, e))
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

        // test case 1: normal
        // execute hook
        let hook = oci::Hook {
            path: "/bin/touch".to_string(),
            args: vec!["touch".to_string(), file_str.to_string()],
            env: vec![],
            timeout: Some(0),
        };
        let ret = execute_hook(&hook, None);
        assert!(ret.is_ok());
        assert!(fs::metadata(&file).is_ok());

        // test case 2: timeout in 10s
        let hook = oci::Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(0), // default timeout is 10 seconds
        };
        let start = Instant::now();
        let ret = execute_hook(&hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((10..12u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);

        // test case 3: timeout in 5s
        let hook = oci::Hook {
            path: "/bin/sleep".to_string(),
            args: vec!["sleep".to_string(), "3600".to_string()],
            env: vec![],
            timeout: Some(5), // timeout is set to 5 seconds
        };
        let start = Instant::now();
        let ret = execute_hook(&hook, None).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((5..7u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);

        // test case 4: with envs
        let shell_path = format!("{}/test.sh", tmpdir.path().to_string_lossy());
        let ret = create_shell(&shell_path, file_str.as_ref());
        assert!(ret.is_ok());
        let hook = oci::Hook {
            path: shell_path,
            args: vec![],
            env: vec!["K1=V1".to_string()],
            timeout: Some(5),
        };
        let ret = execute_hook(&hook, None);
        assert!(ret.is_ok());
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
            timeout: Some(5), // timeout is set to 5 seconds
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
        let ret = execute_hook(&hook, Some(state)).unwrap_err();
        let duration = start.elapsed();
        let used = duration.as_secs();
        assert!((5..7u64).contains(&used));
        assert_eq!(ret.kind(), io::ErrorKind::TimedOut);
    }

    fn create_shell(shell_path: &str, data_path: &str) -> Result<()> {
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
    }
}
