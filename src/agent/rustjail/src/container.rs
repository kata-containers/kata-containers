// Copyright (c) 2019, 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use libc::pid_t;
use oci::{Hook, Linux, LinuxNamespace, LinuxResources, POSIXRlimit, Spec};
use oci::{LinuxDevice, LinuxIDMapping};
use std::clone::Clone;
use std::ffi::{CStr, CString};
use std::fmt;
use std::fmt::Display;
use std::fs;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cgroups::freezer::FreezerState;

use crate::capabilities::{self, CAPSMAP};
#[cfg(not(test))]
use crate::cgroups::fs::Manager as FsManager;
#[cfg(test)]
use crate::cgroups::mock::Manager as FsManager;
use crate::cgroups::Manager;
use crate::log_child;
use crate::process::Process;
use crate::specconv::CreateOpts;
use crate::{mount, validator};

use protocols::agent::StatsContainerResponse;

use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
use nix::fcntl::{FcntlArg, FdFlag};
use nix::mount::MntFlags;
use nix::pty;
use nix::sched::{self, CloneFlags};
use nix::sys::signal::{self, Signal};
use nix::sys::stat::{self, Mode};
use nix::unistd::{self, fork, ForkResult, Gid, Pid, Uid};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;

use protobuf::SingularPtrField;

use oci::State as OCIState;
use std::collections::HashMap;
use std::os::unix::io::FromRawFd;

use slog::{info, o, Logger};

use crate::pipestream::PipeStream;
use crate::sync::{read_sync, write_count, write_sync, SYNC_DATA, SYNC_FAILED, SYNC_SUCCESS};
use crate::sync_with_async::{read_async, write_async};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

const STATE_FILENAME: &str = "state.json";
const EXEC_FIFO_FILENAME: &str = "exec.fifo";
const VER_MARKER: &str = "1.2.5";
const PID_NS_PATH: &str = "/proc/self/ns/pid";

const INIT: &str = "INIT";
const NO_PIVOT: &str = "NO_PIVOT";
const CRFD_FD: &str = "CRFD_FD";
const CWFD_FD: &str = "CWFD_FD";
const CLOG_FD: &str = "CLOG_FD";
const FIFO_FD: &str = "FIFO_FD";
const HOME_ENV_KEY: &str = "HOME";
const PIDNS_FD: &str = "PIDNS_FD";

#[derive(PartialEq, Clone, Copy)]
pub enum Status {
    CREATED,
    RUNNING,
    STOPPED,
    PAUSED,
}

#[derive(Debug)]
pub struct ContainerStatus {
    pre_status: Status,
    cur_status: Status,
}

impl ContainerStatus {
    fn new() -> Self {
        ContainerStatus {
            pre_status: Status::CREATED,
            cur_status: Status::CREATED,
        }
    }

    fn status(&self) -> Status {
        self.cur_status
    }

    fn pre_status(&self) -> Status {
        self.pre_status
    }

    fn transition(&mut self, to: Status) {
        self.pre_status = self.status();
        self.cur_status = to;
    }
}

pub type Config = CreateOpts;
type NamespaceType = String;

impl fmt::Debug for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::CREATED => write!(f, "{:?}", "created"),
            Status::RUNNING => write!(f, "{:?}", "running"),
            Status::STOPPED => write!(f, "{:?}", "stopped"),
            Status::PAUSED => write!(f, "{:?}", "paused"),
        }
    }
}

lazy_static! {
    static ref NAMESPACES: HashMap<&'static str, CloneFlags> = {
        let mut m = HashMap::new();
        m.insert("user", CloneFlags::CLONE_NEWUSER);
        m.insert("ipc", CloneFlags::CLONE_NEWIPC);
        m.insert("pid", CloneFlags::CLONE_NEWPID);
        m.insert("network", CloneFlags::CLONE_NEWNET);
        m.insert("mount", CloneFlags::CLONE_NEWNS);
        m.insert("uts", CloneFlags::CLONE_NEWUTS);
        m.insert("cgroup", CloneFlags::CLONE_NEWCGROUP);
        m
    };

// type to name hashmap, better to be in NAMESPACES
    static ref TYPETONAME: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("ipc", "ipc");
        m.insert("user", "user");
        m.insert("pid", "pid");
        m.insert("network", "net");
        m.insert("mount", "mnt");
        m.insert("cgroup", "cgroup");
        m.insert("uts", "uts");
        m
    };

    pub static ref DEFAULT_DEVICES: Vec<LinuxDevice> = {
        let mut v = Vec::new();
        v.push(LinuxDevice {
            path: "/dev/null".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 3,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/zero".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 5,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/full".to_string(),
            r#type: String::from("c"),
            major: 1,
            minor: 7,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/tty".to_string(),
            r#type: "c".to_string(),
            major: 5,
            minor: 0,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/urandom".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 9,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/random".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 8,
            file_mode: Some(0o666),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BaseState {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    id: String,
    #[serde(default)]
    init_process_pid: i32,
    #[serde(default)]
    init_process_start: u64,
}

#[async_trait]
pub trait BaseContainer {
    fn id(&self) -> String;
    fn status(&self) -> Status;
    fn state(&self) -> Result<State>;
    fn oci_state(&self) -> Result<OCIState>;
    fn config(&self) -> Result<&Config>;
    fn processes(&self) -> Result<Vec<i32>>;
    fn get_process(&mut self, eid: &str) -> Result<&mut Process>;
    fn stats(&self) -> Result<StatsContainerResponse>;
    fn set(&mut self, config: LinuxResources) -> Result<()>;
    async fn start(&mut self, p: Process) -> Result<()>;
    async fn run(&mut self, p: Process) -> Result<()>;
    async fn destroy(&mut self) -> Result<()>;
    fn signal(&self, sig: Signal, all: bool) -> Result<()>;
    fn exec(&mut self) -> Result<()>;
}

// LinuxContainer protected by Mutex
// Arc<Mutex<Innercontainer>> or just Mutex<InnerContainer>?
// Or use Mutex<xx> as a member of struct, like C?
// a lot of String in the struct might be &str
#[derive(Debug)]
pub struct LinuxContainer {
    pub id: String,
    pub root: String,
    pub config: Config,
    pub cgroup_manager: Option<FsManager>,
    pub init_process_pid: pid_t,
    pub init_process_start_time: u64,
    pub uid_map_path: String,
    pub gid_map_path: String,
    pub processes: HashMap<pid_t, Process>,
    pub status: ContainerStatus,
    pub created: SystemTime,
    pub logger: Logger,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    base: BaseState,
    #[serde(default)]
    rootless: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    cgroup_paths: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    namespace_paths: HashMap<NamespaceType, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    external_descriptors: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    intel_rdt_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncPC {
    #[serde(default)]
    pid: pid_t,
}

#[async_trait]
pub trait Container: BaseContainer {
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
}

impl Container for LinuxContainer {
    fn pause(&mut self) -> Result<()> {
        let status = self.status();
        if status != Status::RUNNING && status != Status::CREATED {
            return Err(anyhow!(
                "failed to pause container: current status is: {:?}",
                status
            ));
        }

        if self.cgroup_manager.is_some() {
            self.cgroup_manager
                .as_ref()
                .unwrap()
                .freeze(FreezerState::Frozen)?;

            self.status.transition(Status::PAUSED);
            return Ok(());
        }
        Err(anyhow!("failed to get container's cgroup manager"))
    }

    fn resume(&mut self) -> Result<()> {
        let status = self.status();
        if status != Status::PAUSED {
            return Err(anyhow!("container status is: {:?}, not paused", status));
        }

        if self.cgroup_manager.is_some() {
            self.cgroup_manager
                .as_ref()
                .unwrap()
                .freeze(FreezerState::Thawed)?;

            self.status.transition(Status::RUNNING);
            return Ok(());
        }
        Err(anyhow!("failed to get container's cgroup manager"))
    }
}

pub fn init_child() {
    let cwfd = std::env::var(CWFD_FD).unwrap().parse::<i32>().unwrap();
    let cfd_log = std::env::var(CLOG_FD).unwrap().parse::<i32>().unwrap();

    match do_init_child(cwfd) {
        Ok(_) => log_child!(cfd_log, "temporary parent process exit successfully"),
        Err(e) => {
            log_child!(cfd_log, "temporary parent process exit:child exit: {:?}", e);
            let _ = write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
        }
    }
}

fn do_init_child(cwfd: RawFd) -> Result<()> {
    lazy_static::initialize(&NAMESPACES);
    lazy_static::initialize(&DEFAULT_DEVICES);
    lazy_static::initialize(&RLIMITMAPS);
    lazy_static::initialize(&CAPSMAP);

    let init = std::env::var(INIT)?.eq(format!("{}", true).as_str());

    let no_pivot = std::env::var(NO_PIVOT)?.eq(format!("{}", true).as_str());
    let crfd = std::env::var(CRFD_FD)?.parse::<i32>().unwrap();
    let cfd_log = std::env::var(CLOG_FD)?.parse::<i32>().unwrap();

    // get the pidns fd from parent, if parent had passed the pidns fd,
    // then get it and join in this pidns; otherwise, create a new pidns
    // by unshare from the parent pidns.
    match std::env::var(PIDNS_FD) {
        Ok(fd) => {
            let pidns_fd = fd.parse::<i32>().context("get parent pidns fd")?;
            sched::setns(pidns_fd, CloneFlags::CLONE_NEWPID).context("failed to join pidns")?;
            let _ = unistd::close(pidns_fd);
        }
        Err(_e) => sched::unshare(CloneFlags::CLONE_NEWPID)?,
    }

    match fork() {
        Ok(ForkResult::Parent { child, .. }) => {
            log_child!(
                cfd_log,
                "Continuing execution in temporary process, new child has pid: {:?}",
                child
            );
            let _ = write_sync(cwfd, SYNC_DATA, format!("{}", pid_t::from(child)).as_str());
            // parent return
            return Ok(());
        }
        Ok(ForkResult::Child) => (),
        Err(e) => {
            return Err(anyhow!(format!(
                "failed to fork temporary process: {:?}",
                e
            )));
        }
    }

    log_child!(cfd_log, "child process start run");
    let buf = read_sync(crfd)?;
    let spec_str = std::str::from_utf8(&buf)?;
    let spec: oci::Spec = serde_json::from_str(spec_str)?;

    log_child!(cfd_log, "notify parent to send oci process");
    write_sync(cwfd, SYNC_SUCCESS, "")?;

    let buf = read_sync(crfd)?;
    let process_str = std::str::from_utf8(&buf)?;
    let oci_process: oci::Process = serde_json::from_str(process_str)?;
    log_child!(cfd_log, "notify parent to send cgroup manager");
    write_sync(cwfd, SYNC_SUCCESS, "")?;

    let buf = read_sync(crfd)?;
    let cm_str = std::str::from_utf8(&buf)?;

    let cm: FsManager = serde_json::from_str(cm_str)?;

    let p = if spec.process.is_some() {
        spec.process.as_ref().unwrap()
    } else {
        return Err(anyhow!("didn't find process in Spec"));
    };

    if spec.linux.is_none() {
        return Err(anyhow!("no linux config"));
    }
    let linux = spec.linux.as_ref().unwrap();

    // get namespace vector to join/new
    let nses = get_namespaces(&linux);

    let mut userns = false;
    let mut to_new = CloneFlags::empty();
    let mut to_join = Vec::new();

    for ns in &nses {
        let s = NAMESPACES.get(&ns.r#type.as_str());
        if s.is_none() {
            return Err(anyhow!("invalid ns type"));
        }
        let s = s.unwrap();

        if ns.path.is_empty() {
            // skip the pidns since it has been done in parent process.
            if *s != CloneFlags::CLONE_NEWPID {
                to_new.set(*s, true);
            }
        } else {
            let fd =
                fcntl::open(ns.path.as_str(), OFlag::O_CLOEXEC, Mode::empty()).map_err(|e| {
                    log_child!(
                        cfd_log,
                        "cannot open type: {} path: {}",
                        ns.r#type.clone(),
                        ns.path.clone()
                    );
                    log_child!(cfd_log, "error is : {:?}", e.as_errno());
                    e
                })?;

            if *s != CloneFlags::CLONE_NEWPID {
                to_join.push((*s, fd));
            }
        }
    }

    if to_new.contains(CloneFlags::CLONE_NEWUSER) {
        userns = true;
    }

    if p.oom_score_adj.is_some() {
        log_child!(cfd_log, "write oom score {}", p.oom_score_adj.unwrap());
        fs::write(
            "/proc/self/oom_score_adj",
            p.oom_score_adj.unwrap().to_string().as_bytes(),
        )?;
    }

    // set rlimit
    for rl in p.rlimits.iter() {
        log_child!(cfd_log, "set resource limit: {:?}", rl);
        setrlimit(rl)?;
    }

    //
    // Make the process non-dumpable, to avoid various race conditions that
    // could cause processes in namespaces we're joining to access host
    // resources (or potentially execute code).
    //
    // However, if the number of namespaces we are joining is 0, we are not
    // going to be switching to a different security context. Thus setting
    // ourselves to be non-dumpable only breaks things (like rootless
    // containers), which is the recommendation from the kernel folks.
    //
    // Ref: https://github.com/opencontainers/runc/commit/50a19c6ff828c58e5dab13830bd3dacde268afe5
    //
    if !nses.is_empty() {
        prctl::set_dumpable(false)
            .map_err(|e| anyhow!(e).context("set process non-dumpable failed"))?;
    }

    if userns {
        log_child!(cfd_log, "enter new user namespace");
        sched::unshare(CloneFlags::CLONE_NEWUSER)?;
    }

    log_child!(cfd_log, "notify parent unshare user ns completed");
    // notify parent unshare user ns completed.
    write_sync(cwfd, SYNC_SUCCESS, "")?;
    // wait parent to setup user id mapping.
    log_child!(cfd_log, "wait parent to setup user id mapping");
    read_sync(crfd)?;

    if userns {
        log_child!(cfd_log, "setup user id");
        setid(Uid::from_raw(0), Gid::from_raw(0))?;
    }

    let mut mount_fd = -1;
    let mut bind_device = false;
    for (s, fd) in to_join {
        if s == CloneFlags::CLONE_NEWNS {
            mount_fd = fd;
            continue;
        }

        log_child!(cfd_log, "join namespace {:?}", s);
        sched::setns(fd, s).or_else(|e| {
            if s == CloneFlags::CLONE_NEWUSER {
                if e.as_errno().unwrap() != Errno::EINVAL {
                    let _ = write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
                    return Err(e);
                }

                Ok(())
            } else {
                let _ = write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
                Err(e)
            }
        })?;

        unistd::close(fd)?;

        if s == CloneFlags::CLONE_NEWUSER {
            setid(Uid::from_raw(0), Gid::from_raw(0))?;
            bind_device = true;
        }
    }

    sched::unshare(to_new & !CloneFlags::CLONE_NEWUSER)?;

    if userns {
        bind_device = true;
    }

    if to_new.contains(CloneFlags::CLONE_NEWUTS) {
        unistd::sethostname(&spec.hostname)?;
    }

    let rootfs = spec.root.as_ref().unwrap().path.as_str();
    log_child!(cfd_log, "setup rootfs {}", rootfs);
    let root = fs::canonicalize(rootfs)?;
    let rootfs = root.to_str().unwrap();

    if to_new.contains(CloneFlags::CLONE_NEWNS) {
        // setup rootfs
        mount::init_rootfs(cfd_log, &spec, &cm.paths, &cm.mounts, bind_device)?;
    }

    if init {
        // notify parent to run prestart hooks
        write_sync(cwfd, SYNC_SUCCESS, "")?;
        // wait parent run prestart hooks
        let _ = read_sync(crfd)?;
    }

    if mount_fd != -1 {
        sched::setns(mount_fd, CloneFlags::CLONE_NEWNS)?;
        unistd::close(mount_fd)?;
    }

    if to_new.contains(CloneFlags::CLONE_NEWNS) {
        // unistd::chroot(rootfs)?;
        if no_pivot {
            mount::ms_move_root(rootfs)?;
        } else {
            // pivot root
            mount::pivot_rootfs(rootfs)?;
        }

        // setup sysctl
        set_sysctls(&linux.sysctl)?;
        unistd::chdir("/")?;
    }

    if to_new.contains(CloneFlags::CLONE_NEWNS) {
        mount::finish_rootfs(cfd_log, &spec)?;
    }

    if !oci_process.cwd.is_empty() {
        unistd::chdir(oci_process.cwd.as_str())?;
    }

    let guser = &oci_process.user;

    let uid = Uid::from_raw(guser.uid);
    let gid = Gid::from_raw(guser.gid);

    // only change stdio devices owner when user
    // isn't root.
    if guser.uid != 0 {
        set_stdio_permissions(guser.uid)?;
    }

    setid(uid, gid)?;

    if !guser.additional_gids.is_empty() {
        setgroups(guser.additional_gids.as_slice()).map_err(|e| {
            let _ = write_sync(
                cwfd,
                SYNC_FAILED,
                format!("setgroups failed: {:?}", e).as_str(),
            );

            e
        })?;
    }

    // NoNewPeiviledges, Drop capabilities
    if oci_process.no_new_privileges {
        prctl::set_no_new_privileges(true).map_err(|_| anyhow!("cannot set no new privileges"))?;
    }

    if oci_process.capabilities.is_some() {
        let c = oci_process.capabilities.as_ref().unwrap();
        capabilities::drop_privileges(cfd_log, c)?;
    }

    if init {
        // notify parent to run poststart hooks
        // cfd is closed when return from join_namespaces
        // should retunr cfile instead of cfd?
        write_sync(cwfd, SYNC_SUCCESS, "")?;
    }

    let args = oci_process.args.to_vec();
    let env = oci_process.env.to_vec();

    let mut fifofd = -1;
    if init {
        fifofd = std::env::var(FIFO_FD)?.parse::<i32>().unwrap();
    }

    // cleanup the env inherited from parent
    for (key, _) in env::vars() {
        env::remove_var(key);
    }

    // setup the envs
    for e in env.iter() {
        let v: Vec<&str> = e.splitn(2, '=').collect();
        if v.len() != 2 {
            continue;
        }
        env::set_var(v[0], v[1]);
    }

    // set the "HOME" env getting from "/etc/passwd"
    if env::var_os(HOME_ENV_KEY).is_none() {
        if let Some(home_dir) = dirs::home_dir() {
            env::set_var(HOME_ENV_KEY, home_dir);
        }
    }

    let exec_file = Path::new(&args[0]);
    log_child!(cfd_log, "process command: {:?}", &args);
    if !exec_file.exists() {
        find_file(exec_file).ok_or_else(|| anyhow!("the file {} is not exist", &args[0]))?;
    }

    // notify parent that the child's ready to start
    write_sync(cwfd, SYNC_SUCCESS, "")?;
    log_child!(cfd_log, "ready to run exec");
    let _ = unistd::close(cfd_log);
    let _ = unistd::close(crfd);
    let _ = unistd::close(cwfd);

    if oci_process.terminal {
        unistd::setsid()?;
        unsafe {
            libc::ioctl(0, libc::TIOCSCTTY);
        }
    }

    if init {
        let fd = fcntl::open(
            format!("/proc/self/fd/{}", fifofd).as_str(),
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(0),
        )?;
        unistd::close(fifofd)?;
        let mut buf: &mut [u8] = &mut [0];
        unistd::read(fd, &mut buf)?;
    }

    do_exec(&args);
}

// set_stdio_permissions fixes the permissions of PID 1's STDIO
// within the container to the specified user.
// The ownership needs to match because it is created outside of
// the container and needs to be localized.
fn set_stdio_permissions(uid: libc::uid_t) -> Result<()> {
    let meta = fs::metadata("/dev/null")?;
    let fds = [
        std::io::stdin().as_raw_fd(),
        std::io::stdout().as_raw_fd(),
        std::io::stderr().as_raw_fd(),
    ];

    for fd in &fds {
        let stat = stat::fstat(*fd)?;
        // Skip chown of /dev/null if it was used as one of the STDIO fds.
        if stat.st_rdev == meta.rdev() {
            continue;
        }

        // According to the POSIX specification, -1 is used to indicate that owner and group
        // are not to be changed.  Since uid_t and gid_t are unsigned types, we have to wrap
        // around to get -1.
        let gid = (0 as libc::gid_t).wrapping_sub(1);

        // We only change the uid owner (as it is possible for the mount to
        // prefer a different gid, and there's no reason for us to change it).
        // The reason why we don't just leave the default uid=X mount setup is
        // that users expect to be able to actually use their console. Without
        // this code, you couldn't effectively run as a non-root user inside a
        // container and also have a console set up.
        let res = unsafe { libc::fchown(*fd, uid, gid) };
        Errno::result(res).map_err(|e| anyhow!(e).context("set stdio permissions failed"))?;
    }

    Ok(())
}

#[async_trait]
impl BaseContainer for LinuxContainer {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn status(&self) -> Status {
        self.status.status()
    }

    fn state(&self) -> Result<State> {
        Err(anyhow!("not supported"))
    }

    fn oci_state(&self) -> Result<OCIState> {
        let oci = match self.config.spec.as_ref() {
            Some(s) => s,
            None => return Err(anyhow!("Unable to get OCI state: spec not found")),
        };

        let status = self.status();
        let pid = if status != Status::STOPPED {
            self.init_process_pid
        } else {
            0
        };

        let root = match oci.root.as_ref() {
            Some(s) => s.path.as_str(),
            None => return Err(anyhow!("Unable to get root path: oci.root is none")),
        };

        let path = fs::canonicalize(root)?;
        let bundle = match path.parent() {
            Some(s) => s.to_str().unwrap().to_string(),
            None => return Err(anyhow!("could not get root parent: root path {:?}", path)),
        };

        Ok(OCIState {
            version: oci.version.clone(),
            id: self.id(),
            status: format!("{:?}", status),
            pid,
            bundle,
            annotations: oci.annotations.clone(),
        })
    }

    fn config(&self) -> Result<&Config> {
        Ok(&self.config)
    }

    fn processes(&self) -> Result<Vec<i32>> {
        Ok(self.processes.keys().cloned().collect())
    }

    fn get_process(&mut self, eid: &str) -> Result<&mut Process> {
        for (_, v) in self.processes.iter_mut() {
            if eid == v.exec_id.as_str() {
                return Ok(v);
            }
        }

        Err(anyhow!("invalid eid {}", eid))
    }

    fn stats(&self) -> Result<StatsContainerResponse> {
        let mut r = StatsContainerResponse::default();

        if self.cgroup_manager.is_some() {
            r.cgroup_stats =
                SingularPtrField::some(self.cgroup_manager.as_ref().unwrap().get_stats()?);
        }

        // what about network interface stats?

        Ok(r)
    }

    fn set(&mut self, r: LinuxResources) -> Result<()> {
        if self.cgroup_manager.is_some() {
            self.cgroup_manager.as_ref().unwrap().set(&r, true)?;
        }
        self.config
            .spec
            .as_mut()
            .unwrap()
            .linux
            .as_mut()
            .unwrap()
            .resources = Some(r);
        Ok(())
    }

    async fn start(&mut self, mut p: Process) -> Result<()> {
        let logger = self.logger.new(o!("eid" => p.exec_id.clone()));
        let tty = p.tty;
        let fifo_file = format!("{}/{}", &self.root, EXEC_FIFO_FILENAME);
        info!(logger, "enter container.start!");
        let mut fifofd: RawFd = -1;
        if p.init {
            if stat::stat(fifo_file.as_str()).is_ok() {
                return Err(anyhow!("exec fifo exists"));
            }
            unistd::mkfifo(fifo_file.as_str(), Mode::from_bits(0o622).unwrap())?;

            fifofd = fcntl::open(
                fifo_file.as_str(),
                OFlag::O_PATH,
                Mode::from_bits(0).unwrap(),
            )?;
        }
        info!(logger, "exec fifo opened!");

        if self.config.spec.is_none() {
            return Err(anyhow!("no spec"));
        }

        let spec = self.config.spec.as_ref().unwrap();
        if spec.linux.is_none() {
            return Err(anyhow!("no linux config"));
        }
        let linux = spec.linux.as_ref().unwrap();

        let st = self.oci_state()?;

        let (pfd_log, cfd_log) = unistd::pipe().context("failed to create pipe")?;

        let _ = fcntl::fcntl(pfd_log, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
            .map_err(|e| warn!(logger, "fcntl pfd log FD_CLOEXEC {:?}", e));

        let child_logger = logger.new(o!("action" => "child process log"));
        let log_handler = setup_child_logger(pfd_log, child_logger);

        let (prfd, cwfd) = unistd::pipe().context("failed to create pipe")?;
        let (crfd, pwfd) = unistd::pipe().context("failed to create pipe")?;

        let _ = fcntl::fcntl(prfd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
            .map_err(|e| warn!(logger, "fcntl prfd FD_CLOEXEC {:?}", e));

        let _ = fcntl::fcntl(pwfd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
            .map_err(|e| warn!(logger, "fcntl pwfd FD_COLEXEC {:?}", e));

        let mut pipe_r = PipeStream::from_fd(prfd);
        let mut pipe_w = PipeStream::from_fd(pwfd);

        let child_stdin: std::process::Stdio;
        let child_stdout: std::process::Stdio;
        let child_stderr: std::process::Stdio;

        if tty {
            let pseudo = pty::openpty(None, None)?;
            p.term_master = Some(pseudo.master);
            let _ = fcntl::fcntl(pseudo.master, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
                .map_err(|e| warn!(logger, "fnctl pseudo.master {:?}", e));
            let _ = fcntl::fcntl(pseudo.slave, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
                .map_err(|e| warn!(logger, "fcntl pseudo.slave {:?}", e));

            child_stdin = unsafe { std::process::Stdio::from_raw_fd(pseudo.slave) };
            child_stdout = unsafe { std::process::Stdio::from_raw_fd(pseudo.slave) };
            child_stderr = unsafe { std::process::Stdio::from_raw_fd(pseudo.slave) };
        } else {
            let stdin = p.stdin.unwrap();
            let stdout = p.stdout.unwrap();
            let stderr = p.stderr.unwrap();
            child_stdin = unsafe { std::process::Stdio::from_raw_fd(stdin) };
            child_stdout = unsafe { std::process::Stdio::from_raw_fd(stdout) };
            child_stderr = unsafe { std::process::Stdio::from_raw_fd(stderr) };
        }

        let pidns = get_pid_namespace(&self.logger, linux)?;

        defer!(if let Some(pid) = pidns {
            let _ = unistd::close(pid);
        });

        let exec_path = std::env::current_exe()?;
        let mut child = std::process::Command::new(exec_path);
        let mut child = child
            .arg("init")
            .stdin(child_stdin)
            .stdout(child_stdout)
            .stderr(child_stderr)
            .env(INIT, format!("{}", p.init))
            .env(NO_PIVOT, format!("{}", self.config.no_pivot_root))
            .env(CRFD_FD, format!("{}", crfd))
            .env(CWFD_FD, format!("{}", cwfd))
            .env(CLOG_FD, format!("{}", cfd_log));

        if p.init {
            child = child.env(FIFO_FD, format!("{}", fifofd));
        }

        if pidns.is_some() {
            child = child.env(PIDNS_FD, format!("{}", pidns.unwrap()));
        }

        let child = child.spawn()?;

        unistd::close(crfd)?;
        unistd::close(cwfd)?;
        unistd::close(cfd_log)?;

        // get container process's pid
        let pid_buf = read_async(&mut pipe_r).await?;
        let pid_str = std::str::from_utf8(&pid_buf).context("get pid string")?;
        let pid = match pid_str.parse::<i32>() {
            Ok(i) => i,
            Err(e) => {
                return Err(anyhow!(format!(
                    "failed to get container process's pid: {:?}",
                    e
                )));
            }
        };

        p.pid = pid;

        if p.init {
            self.init_process_pid = p.pid;
        }

        if p.init {
            let _ = unistd::close(fifofd).map_err(|e| warn!(logger, "close fifofd {:?}", e));
        }

        info!(logger, "child pid: {}", p.pid);

        join_namespaces(
            &logger,
            &spec,
            &p,
            self.cgroup_manager.as_ref().unwrap(),
            &st,
            &mut pipe_w,
            &mut pipe_r,
        )
        .await
        .map_err(|e| {
            error!(logger, "create container process error {:?}", e);
            // kill the child process.
            let _ = signal::kill(Pid::from_raw(p.pid), Some(Signal::SIGKILL))
                .map_err(|e| warn!(logger, "signal::kill joining namespaces {:?}", e));

            e
        })?;

        info!(logger, "entered namespaces!");

        self.created = SystemTime::now();

        // create the pipes for notify process exited
        let (exit_pipe_r, exit_pipe_w) = unistd::pipe2(OFlag::O_CLOEXEC)
            .context("failed to create pipe")
            .map_err(|e| {
                let _ = signal::kill(Pid::from_raw(child.id() as i32), Some(Signal::SIGKILL))
                    .map_err(|e| warn!(logger, "signal::kill creating pipe {:?}", e));

                e
            })?;

        p.exit_pipe_w = Some(exit_pipe_w);
        p.exit_pipe_r = Some(exit_pipe_r);

        if p.init {
            let spec = self.config.spec.as_mut().unwrap();
            update_namespaces(&self.logger, spec, p.pid)?;
        }
        self.processes.insert(p.pid, p);

        info!(logger, "wait on child log handler");
        let _ = log_handler
            .await
            .map_err(|e| warn!(logger, "joining log handler {:?}", e));
        info!(logger, "create process completed");
        Ok(())
    }

    async fn run(&mut self, p: Process) -> Result<()> {
        let init = p.init;
        self.start(p).await?;

        if init {
            self.exec()?;
            self.status.transition(Status::RUNNING);
        }

        Ok(())
    }

    async fn destroy(&mut self) -> Result<()> {
        let spec = self.config.spec.as_ref().unwrap();
        let st = self.oci_state()?;

        for pid in self.processes.keys() {
            signal::kill(Pid::from_raw(*pid), Some(Signal::SIGKILL))?;
        }

        if spec.hooks.is_some() {
            info!(self.logger, "poststop");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.poststop.iter() {
                execute_hook(&self.logger, h, &st).await?;
            }
        }

        self.status.transition(Status::STOPPED);
        mount::umount2(
            spec.root.as_ref().unwrap().path.as_str(),
            MntFlags::MNT_DETACH,
        )?;
        fs::remove_dir_all(&self.root)?;

        if let Some(cgm) = self.cgroup_manager.as_mut() {
            cgm.destroy().context("destroy cgroups")?;
        }
        Ok(())
    }

    fn signal(&self, sig: Signal, all: bool) -> Result<()> {
        if all {
            for pid in self.processes.keys() {
                signal::kill(Pid::from_raw(*pid), Some(sig))?;
            }
        }

        signal::kill(Pid::from_raw(self.init_process_pid), Some(sig))?;

        Ok(())
    }

    fn exec(&mut self) -> Result<()> {
        let fifo = format!("{}/{}", &self.root, EXEC_FIFO_FILENAME);
        let fd = fcntl::open(fifo.as_str(), OFlag::O_WRONLY, Mode::from_bits_truncate(0))?;
        let data: &[u8] = &[0];
        unistd::write(fd, &data)?;
        info!(self.logger, "container started");
        self.init_process_start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.status.transition(Status::RUNNING);
        unistd::close(fd)?;

        Ok(())
    }
}

use std::env;

fn find_file<P>(exe_name: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .filter_map(|dir| {
                let full_path = dir.join(&exe_name);
                if full_path.is_file() {
                    Some(full_path)
                } else {
                    None
                }
            })
            .next()
    })
}

fn do_exec(args: &[String]) -> ! {
    let path = &args[0];
    let p = CString::new(path.to_string()).unwrap();
    let sa: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();
    let a: Vec<&CStr> = sa.iter().map(|s| s.as_c_str()).collect();

    let _ = unistd::execvp(p.as_c_str(), a.as_slice()).map_err(|e| match e {
        nix::Error::Sys(errno) => {
            std::process::exit(errno as i32);
        }
        _ => std::process::exit(-2),
    });

    unreachable!()
}

fn update_namespaces(logger: &Logger, spec: &mut Spec, init_pid: RawFd) -> Result<()> {
    info!(logger, "updating namespaces");
    let linux = spec
        .linux
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;

    let namespaces = linux.namespaces.as_mut_slice();
    for namespace in namespaces.iter_mut() {
        if TYPETONAME.contains_key(namespace.r#type.as_str()) {
            let ns_path = format!(
                "/proc/{}/ns/{}",
                init_pid,
                TYPETONAME.get(namespace.r#type.as_str()).unwrap()
            );

            if namespace.path == "" {
                namespace.path = ns_path;
            }
        }
    }

    Ok(())
}

fn get_pid_namespace(logger: &Logger, linux: &Linux) -> Result<Option<RawFd>> {
    for ns in &linux.namespaces {
        if ns.r#type == "pid" {
            if ns.path == "" {
                return Ok(None);
            }

            let fd =
                fcntl::open(ns.path.as_str(), OFlag::O_RDONLY, Mode::empty()).map_err(|e| {
                    error!(
                        logger,
                        "cannot open type: {} path: {}",
                        ns.r#type.clone(),
                        ns.path.clone()
                    );
                    error!(logger, "error is : {:?}", e.as_errno());

                    e
                })?;

            return Ok(Some(fd));
        }
    }

    Err(anyhow!("cannot find the pid ns"))
}

fn is_userns_enabled(linux: &Linux) -> bool {
    linux
        .namespaces
        .iter()
        .any(|ns| ns.r#type == "user" && ns.path == "")
}

fn get_namespaces(linux: &Linux) -> Vec<LinuxNamespace> {
    linux
        .namespaces
        .iter()
        .map(|ns| LinuxNamespace {
            r#type: ns.r#type.clone(),
            path: ns.path.clone(),
        })
        .collect()
}

pub fn setup_child_logger(fd: RawFd, child_logger: Logger) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let log_file_stream = PipeStream::from_fd(fd);
        let buf_reader_stream = tokio::io::BufReader::new(log_file_stream);
        let mut lines = buf_reader_stream.lines();

        loop {
            match lines.next_line().await {
                Err(e) => {
                    info!(child_logger, "read child process log error: {:?}", e);
                    break;
                }
                Ok(Some(line)) => {
                    info!(child_logger, "{}", line);
                }
                Ok(None) => {
                    info!(child_logger, "read child process log end",);
                    break;
                }
            }
        }
    })
}

async fn join_namespaces(
    logger: &Logger,
    spec: &Spec,
    p: &Process,
    cm: &FsManager,
    st: &OCIState,
    pipe_w: &mut PipeStream,
    pipe_r: &mut PipeStream,
) -> Result<()> {
    let logger = logger.new(o!("action" => "join-namespaces"));

    let linux = spec.linux.as_ref().unwrap();
    let res = linux.resources.as_ref();

    let userns = is_userns_enabled(linux);

    info!(logger, "try to send spec from parent to child");
    let spec_str = serde_json::to_string(spec)?;
    write_async(pipe_w, SYNC_DATA, spec_str.as_str()).await?;

    info!(logger, "wait child received oci spec");

    read_async(pipe_r).await?;

    info!(logger, "send oci process from parent to child");
    let process_str = serde_json::to_string(&p.oci)?;
    write_async(pipe_w, SYNC_DATA, process_str.as_str()).await?;

    info!(logger, "wait child received oci process");
    read_async(pipe_r).await?;

    let cm_str = serde_json::to_string(cm)?;
    write_async(pipe_w, SYNC_DATA, cm_str.as_str()).await?;

    // wait child setup user namespace
    info!(logger, "wait child setup user namespace");
    read_async(pipe_r).await?;

    if userns {
        info!(logger, "setup uid/gid mappings");
        // setup uid/gid mappings
        write_mappings(
            &logger,
            &format!("/proc/{}/uid_map", p.pid),
            &linux.uid_mappings,
        )?;
        write_mappings(
            &logger,
            &format!("/proc/{}/gid_map", p.pid),
            &linux.gid_mappings,
        )?;
    }

    // apply cgroups
    if p.init && res.is_some() {
        info!(logger, "apply cgroups!");
        cm.set(res.unwrap(), false)?;
    }

    if res.is_some() {
        cm.apply(p.pid)?;
    }

    info!(logger, "notify child to continue");
    // notify child to continue
    write_async(pipe_w, SYNC_SUCCESS, "").await?;

    if p.init {
        info!(logger, "notify child parent ready to run prestart hook!");
        let _ = read_async(pipe_r).await?;

        info!(logger, "get ready to run prestart hook!");

        // run prestart hook
        if spec.hooks.is_some() {
            info!(logger, "prestart hook");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.prestart.iter() {
                execute_hook(&logger, h, st).await?;
            }
        }

        // notify child run prestart hooks completed
        info!(logger, "notify child run prestart hook completed!");
        write_async(pipe_w, SYNC_SUCCESS, "").await?;

        info!(logger, "notify child parent ready to run poststart hook!");
        // wait to run poststart hook
        read_async(pipe_r).await?;
        info!(logger, "get ready to run poststart hook!");

        // run poststart hook
        if spec.hooks.is_some() {
            info!(logger, "poststart hook");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.poststart.iter() {
                execute_hook(&logger, h, st).await?;
            }
        }
    }

    info!(logger, "wait for child process ready to run exec");
    read_async(pipe_r).await?;

    Ok(())
}

fn write_mappings(logger: &Logger, path: &str, maps: &[LinuxIDMapping]) -> Result<()> {
    let data = maps
        .iter()
        .filter(|m| m.size != 0)
        .map(|m| format!("{} {} {}\n", m.container_id, m.host_id, m.size))
        .collect::<Vec<_>>()
        .join("");

    info!(logger, "mapping: {}", data);
    if !data.is_empty() {
        let fd = fcntl::open(path, OFlag::O_WRONLY, Mode::empty())?;
        defer!(unistd::close(fd).unwrap());
        unistd::write(fd, data.as_bytes()).map_err(|e| {
            info!(logger, "cannot write mapping");
            e
        })?;
    }
    Ok(())
}

fn setid(uid: Uid, gid: Gid) -> Result<()> {
    // set uid/gid
    prctl::set_keep_capabilities(true)
        .map_err(|e| anyhow!(e).context("set keep capabilities returned"))?;

    {
        unistd::setresgid(gid, gid, gid)?;
    }
    {
        unistd::setresuid(uid, uid, uid)?;
    }
    // if we change from zero, we lose effective caps
    if uid != Uid::from_raw(0) {
        capabilities::reset_effective()?;
    }

    prctl::set_keep_capabilities(false)
        .map_err(|e| anyhow!(e).context("set keep capabilities returned"))?;

    Ok(())
}

impl LinuxContainer {
    pub fn new<T: Into<String> + Display + Clone>(
        id: T,
        base: T,
        config: Config,
        logger: &Logger,
    ) -> Result<Self> {
        let base = base.into();
        let id = id.into();
        let root = format!("{}/{}", base.as_str(), id.as_str());

        // validate oci spec
        validator::validate(&config)?;

        fs::create_dir_all(root.as_str()).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                return anyhow!(e).context(format!("container {} already exists", id.as_str()));
            }

            anyhow!(e).context(format!("fail to create container directory {}", root))
        })?;

        unistd::chown(
            root.as_str(),
            Some(unistd::getuid()),
            Some(unistd::getgid()),
        )
        .context(format!("cannot change onwer of container {} root", id))?;

        if config.spec.is_none() {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }

        let spec = config.spec.as_ref().unwrap();

        if spec.linux.is_none() {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }

        let linux = spec.linux.as_ref().unwrap();

        let cpath = if linux.cgroups_path.is_empty() {
            format!("/{}", id.as_str())
        } else {
            linux.cgroups_path.clone()
        };

        let cgroup_manager = FsManager::new(cpath.as_str())?;
        info!(logger, "new cgroup_manager {:?}", &cgroup_manager);

        Ok(LinuxContainer {
            id: id.clone(),
            root,
            cgroup_manager: Some(cgroup_manager),
            status: ContainerStatus::new(),
            uid_map_path: String::from(""),
            gid_map_path: "".to_string(),
            config,
            processes: HashMap::new(),
            created: SystemTime::now(),
            init_process_pid: -1,
            init_process_start_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            logger: logger.new(o!("module" => "rustjail", "subsystem" => "container", "cid" => id)),
        })
    }

    fn load<T: Into<String>>(_id: T, _base: T) -> Result<Self> {
        Err(anyhow!("not supported"))
    }
}

// Handle the differing rlimit types for different targets
#[cfg(target_env = "musl")]
type RlimitsType = libc::c_int;
#[cfg(target_env = "gnu")]
type RlimitsType = libc::__rlimit_resource_t;

lazy_static! {
    pub static ref RLIMITMAPS: HashMap<String, RlimitsType> = {
        let mut m = HashMap::new();
        m.insert("RLIMIT_CPU".to_string(), libc::RLIMIT_CPU);
        m.insert("RLIMIT_FSIZE".to_string(), libc::RLIMIT_FSIZE);
        m.insert("RLIMIT_DATA".to_string(), libc::RLIMIT_DATA);
        m.insert("RLIMIT_STACK".to_string(), libc::RLIMIT_STACK);
        m.insert("RLIMIT_CORE".to_string(), libc::RLIMIT_CORE);
        m.insert("RLIMIT_RSS".to_string(), libc::RLIMIT_RSS);
        m.insert("RLIMIT_NPROC".to_string(), libc::RLIMIT_NPROC);
        m.insert("RLIMIT_NOFILE".to_string(), libc::RLIMIT_NOFILE);
        m.insert("RLIMIT_MEMLOCK".to_string(), libc::RLIMIT_MEMLOCK);
        m.insert("RLIMIT_AS".to_string(), libc::RLIMIT_AS);
        m.insert("RLIMIT_LOCKS".to_string(), libc::RLIMIT_LOCKS);
        m.insert("RLIMIT_SIGPENDING".to_string(), libc::RLIMIT_SIGPENDING);
        m.insert("RLIMIT_MSGQUEUE".to_string(), libc::RLIMIT_MSGQUEUE);
        m.insert("RLIMIT_NICE".to_string(), libc::RLIMIT_NICE);
        m.insert("RLIMIT_RTPRIO".to_string(), libc::RLIMIT_RTPRIO);
        m.insert("RLIMIT_RTTIME".to_string(), libc::RLIMIT_RTTIME);
        m
    };
}

fn setrlimit(limit: &POSIXRlimit) -> Result<()> {
    let rl = libc::rlimit {
        rlim_cur: limit.soft,
        rlim_max: limit.hard,
    };

    let res = if RLIMITMAPS.get(limit.r#type.as_str()).is_some() {
        *RLIMITMAPS.get(limit.r#type.as_str()).unwrap()
    } else {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    };

    let ret = unsafe { libc::setrlimit(res as RlimitsType, &rl as *const libc::rlimit) };

    Errno::result(ret).map(drop)?;

    Ok(())
}

fn setgroups(grps: &[libc::gid_t]) -> Result<()> {
    let ret = unsafe { libc::setgroups(grps.len(), grps.as_ptr() as *const libc::gid_t) };
    Errno::result(ret).map(drop)?;
    Ok(())
}

use std::fs::OpenOptions;
use std::io::Write;

fn set_sysctls(sysctls: &HashMap<String, String>) -> Result<()> {
    for (key, value) in sysctls {
        let name = format!("/proc/sys/{}", key.replace('.', "/"));
        let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(name.as_str())
        {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                return Err(e.into());
            }
        };

        file.write_all(value.as_bytes())?;
    }

    Ok(())
}

use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::time::Duration;

async fn execute_hook(logger: &Logger, h: &Hook, st: &OCIState) -> Result<()> {
    let logger = logger.new(o!("action" => "execute-hook"));

    let binary = PathBuf::from(h.path.as_str());
    let path = binary.canonicalize()?;
    if !path.exists() {
        return Err(anyhow!(nix::Error::from_errno(Errno::EINVAL)));
    }

    let args = h.args.clone();
    let envs = h.env.clone();
    let state = serde_json::to_string(st)?;

    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
    defer!({
        let _ = unistd::close(rfd);
        let _ = unistd::close(wfd);
    });

    let mut pipe_r = PipeStream::from_fd(rfd);
    let mut pipe_w = PipeStream::from_fd(wfd);

    match unistd::fork()? {
        ForkResult::Parent { child } => {
            let buf = read_async(&mut pipe_r).await?;
            let status = if buf.len() == 4 {
                let buf_array: [u8; 4] = [buf[0], buf[1], buf[2], buf[3]];
                i32::from_be_bytes(buf_array)
            } else {
                -libc::EPIPE
            };

            info!(logger, "hook child: {} status: {}", child, status);

            if status != 0 {
                if status == -libc::ETIMEDOUT {
                    return Err(anyhow!(nix::Error::from_errno(Errno::ETIMEDOUT)));
                } else if status == -libc::EPIPE {
                    return Err(anyhow!(nix::Error::from_errno(Errno::EPIPE)));
                } else {
                    return Err(anyhow!(nix::Error::from_errno(Errno::UnknownErrno)));
                }
            }

            Ok(())
        }

        ForkResult::Child => {
            let (mut tx, mut rx) = tokio::sync::mpsc::channel(100);
            let (tx_logger, rx_logger) = tokio::sync::oneshot::channel();

            tx_logger.send(logger.clone()).unwrap();

            let handle = tokio::spawn(async move {
                let logger = rx_logger.await.unwrap();

                // write oci state to child
                let env: HashMap<String, String> = envs
                    .iter()
                    .map(|e| {
                        let v: Vec<&str> = e.split('=').collect();
                        (v[0].to_string(), v[1].to_string())
                    })
                    .collect();

                let mut child = tokio::process::Command::new(path.to_str().unwrap())
                    .args(args.iter())
                    .envs(env.iter())
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();

                // send out our pid
                tx.send(child.id() as libc::pid_t).await.unwrap();
                info!(logger, "hook grand: {}", child.id());

                child
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(state.as_bytes())
                    .await
                    .unwrap();

                // read something from stdout for debug
                let mut out = String::new();
                child
                    .stdout
                    .as_mut()
                    .unwrap()
                    .read_to_string(&mut out)
                    .await
                    .unwrap();
                info!(logger, "child stdout: {}", out.as_str());
                match child.await {
                    Ok(exit) => {
                        let code: i32 = if exit.success() {
                            0
                        } else {
                            match exit.code() {
                                Some(c) => (c as u32 | 0x80000000) as i32,
                                None => exit.signal().unwrap(),
                            }
                        };

                        tx.send(code).await.unwrap();
                    }

                    Err(e) => {
                        info!(
                            logger,
                            "wait child error: {} {}",
                            e,
                            e.raw_os_error().unwrap()
                        );

                        // There is apparently race between this wait and
                        // child reaper. Ie, the child can already
                        // be reaped by subreaper, child.wait returns
                        // ECHILD. I have no idea how to get the
                        // correct exit status here at present,
                        // just pretend it exits successfully.
                        // -- FIXME
                        // just in case. Should not happen any more

                        tx.send(0).await.unwrap();
                    }
                }
            });

            let pid = rx.recv().await.unwrap();
            info!(logger, "hook grand: {}", pid);

            let status = {
                if let Some(timeout) = h.timeout {
                    let timeout = tokio::time::delay_for(Duration::from_secs(timeout as u64));
                    tokio::select! {
                        v = rx.recv() => {
                            match v {
                                Some(s) => s,
                                None =>  {
                                    let _ = signal::kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                                    -libc::EPIPE
                                }
                            }
                        }
                        _ = timeout => {
                            let _ = signal::kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                            -libc::ETIMEDOUT
                        }
                    }
                } else if let Some(s) = rx.recv().await {
                    s
                } else {
                    let _ = signal::kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                    -libc::EPIPE
                }
            };

            handle.await.unwrap();
            let _ = write_async(
                &mut pipe_w,
                SYNC_DATA,
                std::str::from_utf8(&status.to_be_bytes()).unwrap_or_default(),
            )
            .await;
            std::process::exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::Process;
    use crate::skip_if_not_root;
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::AsRawFd;
    use tempfile::tempdir;

    macro_rules! sl {
        () => {
            slog_scope::logger()
        };
    }

    #[test]
    fn test_status_transtition() {
        let mut status = ContainerStatus::new();
        let status_table: [Status; 4] = [
            Status::CREATED,
            Status::RUNNING,
            Status::PAUSED,
            Status::STOPPED,
        ];

        for s in status_table.iter() {
            let pre_status = status.status();
            status.transition(*s);

            assert_eq!(pre_status, status.pre_status());
        }
    }

    #[test]
    fn test_set_stdio_permissions() {
        skip_if_not_root!();

        let meta = fs::metadata("/dev/stdin").unwrap();
        let old_uid = meta.uid();

        let uid = 1000;
        set_stdio_permissions(uid).unwrap();

        let meta = fs::metadata("/dev/stdin").unwrap();
        assert_eq!(meta.uid(), uid);

        let meta = fs::metadata("/dev/stdout").unwrap();
        assert_eq!(meta.uid(), uid);

        let meta = fs::metadata("/dev/stderr").unwrap();
        assert_eq!(meta.uid(), uid);

        // restore the uid
        set_stdio_permissions(old_uid).unwrap();
    }

    #[test]
    fn test_status_fmt() {
        assert_eq!("\"created\"", format!("{:?}", Status::CREATED));
        assert_eq!("\"running\"", format!("{:?}", Status::RUNNING));
        assert_eq!("\"paused\"", format!("{:?}", Status::PAUSED));
        assert_eq!("\"stopped\"", format!("{:?}", Status::STOPPED));
    }

    #[test]
    fn test_namespaces() {
        lazy_static::initialize(&NAMESPACES);
        assert_eq!(NAMESPACES.len(), 7);

        let ns = NAMESPACES.get("user");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("ipc");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("pid");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("network");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("mount");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("uts");
        assert!(ns.is_some());

        let ns = NAMESPACES.get("cgroup");
        assert!(ns.is_some());
    }

    #[test]
    fn test_typetoname() {
        lazy_static::initialize(&TYPETONAME);
        assert_eq!(TYPETONAME.len(), 7);

        let ns = TYPETONAME.get("user");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("ipc");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("pid");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("network");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("mount");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("uts");
        assert!(ns.is_some());

        let ns = TYPETONAME.get("cgroup");
        assert!(ns.is_some());
    }

    fn create_dummy_opts() -> CreateOpts {
        let mut root = oci::Root::default();
        root.path = "/tmp".to_string();

        let linux = Linux::default();
        let mut spec = Spec::default();
        spec.root = Some(root).into();
        spec.linux = Some(linux).into();

        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
        }
    }

    fn new_linux_container() -> (Result<LinuxContainer>, tempfile::TempDir) {
        // Create a temporal directory
        let dir = tempdir()
            .map_err(|e| anyhow!(e).context("tempdir failed"))
            .unwrap();

        // Create a new container
        (
            LinuxContainer::new(
                "some_id",
                &dir.path().join("rootfs").to_str().unwrap(),
                create_dummy_opts(),
                &slog_scope::logger(),
            ),
            dir,
        )
    }

    fn new_linux_container_and_then<U, F: FnOnce(LinuxContainer) -> Result<U, anyhow::Error>>(
        op: F,
    ) -> Result<U, anyhow::Error> {
        let (container, _dir) = new_linux_container();
        container.and_then(op)
    }

    #[test]
    fn test_linuxcontainer_pause_bad_status() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            // Change state to pause, c.pause() should fail
            c.status.transition(Status::PAUSED);
            c.pause().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        assert!(format!("{:?}", ret).contains("failed to pause container"))
    }

    #[test]
    fn test_linuxcontainer_pause_cgroupmgr_is_none() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.cgroup_manager = None;
            c.pause().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_pause() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.cgroup_manager = FsManager::new("").ok();
            c.pause().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_resume_bad_status() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            // Change state to created, c.resume() should fail
            c.status.transition(Status::CREATED);
            c.resume().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
        assert!(format!("{:?}", ret).contains("not paused"))
    }

    #[test]
    fn test_linuxcontainer_resume_cgroupmgr_is_none() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.status.transition(Status::PAUSED);
            c.cgroup_manager = None;
            c.resume().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_err(), "Expecting error, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_resume() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.cgroup_manager = FsManager::new("").ok();
            // Change status to paused, this way we can resume it
            c.status.transition(Status::PAUSED);
            c.resume().map_err(|e| anyhow!(e))
        });

        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_state() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| c.state());
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
        assert!(
            format!("{:?}", ret).contains("not supported"),
            "Got: {:?}",
            ret
        )
    }

    #[test]
    fn test_linuxcontainer_oci_state_no_root_parent() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.config.spec.as_mut().unwrap().root.as_mut().unwrap().path = "/".to_string();
            c.oci_state()
        });
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
        assert!(
            format!("{:?}", ret).contains("could not get root parent"),
            "Got: {:?}",
            ret
        )
    }

    #[test]
    fn test_linuxcontainer_oci_state() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| c.oci_state());
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_config() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| Ok(c));
        assert!(ret.is_ok(), "Expecting ok, Got {:?}", ret);
        assert!(
            ret.as_ref().unwrap().config().is_ok(),
            "Expecting ok, Got {:?}",
            ret
        );
    }

    #[test]
    fn test_linuxcontainer_processes() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| c.processes());
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_get_process_not_found() {
        let _ = new_linux_container_and_then(|mut c: LinuxContainer| {
            let p = c.get_process("123");
            assert!(p.is_err(), "Expecting Err, Got {:?}", p);
            Ok(())
        });
    }

    #[test]
    fn test_linuxcontainer_get_process() {
        let _ = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.processes.insert(
                1,
                Process::new(&sl!(), &oci::Process::default(), "123", true, 1).unwrap(),
            );
            let p = c.get_process("123");
            assert!(p.is_ok(), "Expecting Ok, Got {:?}", p);
            Ok(())
        });
    }

    #[test]
    fn test_linuxcontainer_stats() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| c.stats());
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_set() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| {
            c.set(oci::LinuxResources::default())
        });
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[tokio::test]
    async fn test_linuxcontainer_start() {
        let (c, _dir) = new_linux_container();
        let ret = c
            .unwrap()
            .start(Process::new(&sl!(), &oci::Process::default(), "123", true, 1).unwrap())
            .await;
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
    }

    #[tokio::test]
    async fn test_linuxcontainer_run() {
        let (c, _dir) = new_linux_container();
        let ret = c
            .unwrap()
            .run(Process::new(&sl!(), &oci::Process::default(), "123", true, 1).unwrap())
            .await;
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
    }

    #[tokio::test]
    async fn test_linuxcontainer_destroy() {
        let (c, _dir) = new_linux_container();

        let ret = c.unwrap().destroy().await;
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_signal() {
        let ret = new_linux_container_and_then(|c: LinuxContainer| {
            c.signal(nix::sys::signal::SIGCONT, true)
        });
        assert!(ret.is_ok(), "Expecting Ok, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_exec() {
        let ret = new_linux_container_and_then(|mut c: LinuxContainer| c.exec());
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
    }

    #[test]
    fn test_linuxcontainer_do_init_child() {
        let ret = do_init_child(std::io::stdin().as_raw_fd());
        assert!(ret.is_err(), "Expecting Err, Got {:?}", ret);
    }
}
