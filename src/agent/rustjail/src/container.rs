// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use lazy_static;
use oci::{Hook, Linux, LinuxNamespace, LinuxResources, POSIXRlimit, Spec};
use serde_json;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
// use crate::sync::Cond;
use libc::pid_t;
use oci::{LinuxDevice, LinuxIDMapping};
use std::clone::Clone;
use std::fmt::Display;
use std::process::{Child, Command};

// use crate::configs::namespaces::{NamespaceType};
use crate::cgroups::Manager as CgroupManager;
use crate::process::Process;
// use crate::intelrdt::Manager as RdtManager;
use crate::errors::*;
use crate::log_child;
use crate::specconv::CreateOpts;
use crate::sync::*;
// use crate::stats::Stats;
use crate::capabilities::{self, CAPSMAP};
use crate::cgroups::fs::{self as fscgroup, Manager as FsManager};
use crate::{mount, validator};

use protocols::agent::StatsContainerResponse;

use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
use nix::fcntl::{FcntlArg, FdFlag};
use nix::pty;
use nix::sched::{self, CloneFlags};
use nix::sys::signal::{self, Signal};
use nix::sys::stat::{self, Mode};
use nix::unistd::{self, ForkResult, Gid, Pid, Uid};
use nix::Error;

use libc;
use protobuf::SingularPtrField;

use oci::State as OCIState;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::os::unix::io::FromRawFd;

use slog::{debug, info, o, Logger};

const STATE_FILENAME: &'static str = "state.json";
const EXEC_FIFO_FILENAME: &'static str = "exec.fifo";
const VER_MARKER: &'static str = "1.2.5";
const PID_NS_PATH: &str = "/proc/self/ns/pid";

const INIT: &str = "INIT";
const NO_PIVOT: &str = "NO_PIVOT";
const CRFD_FD: &str = "CRFD_FD";
const CWFD_FD: &str = "CWFD_FD";
const CLOG_FD: &str = "CLOG_FD";
const FIFO_FD: &str = "FIFO_FD";

type Status = Option<String>;
pub type Config = CreateOpts;
type NamespaceType = String;

/*
impl Status {
    fn to_string(&self) -> String {
        match *self {
            Some(ref v) => v.to_string(),
            None => "Unknown Status".to_string(),
        }
    }
}
*/

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
            file_mode: Some(0o066),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/zero".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 5,
            file_mode: Some(0o066),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/full".to_string(),
            r#type: String::from("c"),
            major: 1,
            minor: 7,
            file_mode: Some(0o066),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/tty".to_string(),
            r#type: "c".to_string(),
            major: 5,
            minor: 0,
            file_mode: Some(0o066),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/urandom".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 9,
            file_mode: Some(0o066),
            uid: Some(0xffffffff),
            gid: Some(0xffffffff),
        });
        v.push(LinuxDevice {
            path: "/dev/random".to_string(),
            r#type: "c".to_string(),
            major: 1,
            minor: 8,
            file_mode: Some(0o066),
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
    /*
    #[serde(default)]
        created: SystemTime,
        config: Config,
    */
}

pub trait BaseContainer {
    fn id(&self) -> String;
    fn status(&self) -> Result<Status>;
    fn state(&self) -> Result<State>;
    fn oci_state(&self) -> Result<OCIState>;
    fn config(&self) -> Result<&Config>;
    fn processes(&self) -> Result<Vec<i32>>;
    fn get_process(&mut self, eid: &str) -> Result<&mut Process>;
    fn stats(&self) -> Result<StatsContainerResponse>;
    fn set(&mut self, config: LinuxResources) -> Result<()>;
    fn start(&mut self, p: Process) -> Result<()>;
    fn run(&mut self, p: Process) -> Result<()>;
    fn destroy(&mut self) -> Result<()>;
    fn signal(&self, sig: Signal, all: bool) -> Result<()>;
    fn exec(&mut self) -> Result<()>;
}

// LinuxContainer protected by Mutex
// Arc<Mutex<Innercontainer>> or just Mutex<InnerContainer>?
// Or use Mutex<xx> as a member of struct, like C?
// a lot of String in the struct might be &str
#[derive(Debug)]
pub struct LinuxContainer
// where T: CgroupManager
{
    pub id: String,
    pub root: String,
    pub config: Config,
    pub cgroup_manager: Option<FsManager>,
    pub init_process_pid: pid_t,
    pub init_process_start_time: u64,
    pub uid_map_path: String,
    pub gid_map_path: String,
    pub processes: HashMap<pid_t, Process>,
    pub status: Status,
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

pub trait Container: BaseContainer {
    //	fn checkpoint(&self, opts: &CriuOpts) -> Result<()>;
    //	fn restore(&self, p: &Process, opts: &CriuOpts) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
    //	fn notify_oom(&self) -> Result<(Sender, Receiver)>;
    //	fn notify_memory_pressure(&self, lvl: PressureLevel) -> Result<(Sender, Receiver)>;
}

pub fn init_child() {
    let cwfd = std::env::var(CWFD_FD).unwrap().parse::<i32>().unwrap();
    let cfd_log = std::env::var(CLOG_FD).unwrap().parse::<i32>().unwrap();
    match do_init_child(cwfd) {
        Ok(_) => (),
        Err(e) => {
            log_child!(cfd_log, "child exit: {:?}", e);
            write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
            return;
        }
    }

    std::process::exit(-1);
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

    log_child!(cfd_log, "child process start run");
    let buf = read_sync(crfd)?;
    let spec_str = std::str::from_utf8(&buf)?;
    let spec: oci::Spec = serde_json::from_str(spec_str)?;

    log_child!(cfd_log, "notify parent to send oci process");
    write_sync(cwfd, SYNC_SUCCESS, "")?;

    let buf = read_sync(crfd)?;
    let process_str = std::str::from_utf8(&buf)?;
    let mut oci_process: oci::Process = serde_json::from_str(process_str)?;
    log_child!(cfd_log, "notify parent to send cgroup manager");
    write_sync(cwfd, SYNC_SUCCESS, "")?;

    let buf = read_sync(crfd)?;
    let cm_str = std::str::from_utf8(&buf)?;
    let cm: FsManager = serde_json::from_str(cm_str)?;

    let p = if spec.process.is_some() {
        spec.process.as_ref().unwrap()
    } else {
        return Err(ErrorKind::ErrorCode("didn't find process in Spec".to_string()).into());
    };

    if spec.linux.is_none() {
        return Err(ErrorKind::ErrorCode("no linux config".to_string()).into());
    }
    let linux = spec.linux.as_ref().unwrap();

    // get namespace vector to join/new
    let nses = get_namespaces(&linux)?;

    let mut userns = false;
    let mut to_new = CloneFlags::empty();
    let mut to_join = Vec::new();

    for ns in &nses {
        let s = NAMESPACES.get(&ns.r#type.as_str());
        if s.is_none() {
            return Err(ErrorKind::ErrorCode("invalid ns type".to_string()).into());
        }
        let s = s.unwrap();

        if ns.path.is_empty() {
            // skip the pidns since it has been done in parent process.
            if *s != CloneFlags::CLONE_NEWPID {
                to_new.set(*s, true);
            }
        } else {
            let fd = match fcntl::open(ns.path.as_str(), OFlag::O_CLOEXEC, Mode::empty()) {
                Ok(v) => v,
                Err(e) => {
                    log_child!(
                        cfd_log,
                        "cannot open type: {} path: {}",
                        ns.r#type.clone(),
                        ns.path.clone()
                    );
                    log_child!(cfd_log, "error is : {}", e.as_errno().unwrap().desc());
                    return Err(e.into());
                }
            };

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
        if let Err(e) = sched::setns(fd, s) {
            if s == CloneFlags::CLONE_NEWUSER {
                if e.as_errno().unwrap() != Errno::EINVAL {
                    write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
                    return Err(e.into());
                }
            } else {
                write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
                return Err(e.into());
            }
        }
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

    setid(uid, gid)?;

    if guser.additional_gids.len() > 0 {
        setgroups(guser.additional_gids.as_slice()).map_err(|e| {
            write_sync(
                cwfd,
                SYNC_FAILED,
                format!("setgroups failed: {:?}", e).as_str(),
            );
            e
        })?;
    }

    // NoNewPeiviledges, Drop capabilities
    if oci_process.no_new_privileges {
        if let Err(_) = prctl::set_no_new_privileges(true) {
            return Err(ErrorKind::ErrorCode("cannot set no new privileges".to_string()).into());
        }
    }

    if oci_process.capabilities.is_some() {
        let c = oci_process.capabilities.as_ref().unwrap();
        capabilities::drop_priviledges(cfd_log, c)?;
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

    //cleanup the env inherited from parent
    for (key, _) in env::vars() {
        env::remove_var(key);
    }

    // setup the envs
    for e in env.iter() {
        let v: Vec<&str> = e.splitn(2, "=").collect();
        if v.len() != 2 {
            //info!(logger, "incorrect env config!");
            continue;
        }
        env::set_var(v[0], v[1]);
    }

    let exec_file = Path::new(&args[0]);
    log_child!(cfd_log, "process command: {:?}", &args);
    if !exec_file.exists() {
        match find_file(exec_file) {
            Some(_) => (),
            None => {
                return Err(
                    ErrorKind::ErrorCode(format!("the file {} is not exist", &args[0])).into(),
                );
            }
        }
    }

    // notify parent that the child's ready to start
    write_sync(cwfd, SYNC_SUCCESS, "")?;
    log_child!(cfd_log, "ready to run exec");
    unistd::close(cfd_log);
    unistd::close(crfd);
    unistd::close(cwfd);

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

    Err(ErrorKind::ErrorCode("fail to create container".to_string()).into())
}

impl BaseContainer for LinuxContainer {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn status(&self) -> Result<Status> {
        Ok(self.status.clone())
    }

    fn state(&self) -> Result<State> {
        Err(ErrorKind::ErrorCode(String::from("not suppoerted")).into())
    }

    fn oci_state(&self) -> Result<OCIState> {
        let oci = self.config.spec.as_ref().unwrap();
        let status = self.status().unwrap().unwrap();
        let pid = if status != "stopped".to_string() {
            self.init_process_pid
        } else {
            0
        };

        let root = oci.root.as_ref().unwrap().path.as_str();
        let path = fs::canonicalize(root)?;
        let bundle = path.parent().unwrap().to_str().unwrap().to_string();
        Ok(OCIState {
            version: oci.version.clone(),
            id: self.id(),
            status,
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

        Err(ErrorKind::ErrorCode(format!("invalid eid {}", eid)).into())
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

    fn start(&mut self, mut p: Process) -> Result<()> {
        let logger = self.logger.new(o!("eid" => p.exec_id.clone()));
        let tty = p.tty;
        let fifo_file = format!("{}/{}", &self.root, EXEC_FIFO_FILENAME);
        info!(logger, "enter container.start!");
        let mut fifofd: RawFd = -1;
        if p.init {
            if let Ok(_) = stat::stat(fifo_file.as_str()) {
                return Err(ErrorKind::ErrorCode("exec fifo exists".to_string()).into());
            }
            unistd::mkfifo(fifo_file.as_str(), Mode::from_bits(0o622).unwrap())?;
            // defer!(fs::remove_file(&fifo_file)?);

            fifofd = fcntl::open(
                fifo_file.as_str(),
                OFlag::O_PATH,
                Mode::from_bits(0).unwrap(),
            )?;
        }
        info!(logger, "exec fifo opened!");

        fscgroup::init_static();

        if self.config.spec.is_none() {
            return Err(ErrorKind::ErrorCode("no spec".to_string()).into());
        }

        let spec = self.config.spec.as_ref().unwrap();
        if spec.linux.is_none() {
            return Err(ErrorKind::ErrorCode("no linux config".to_string()).into());
        }
        let linux = spec.linux.as_ref().unwrap();

        let st = self.oci_state()?;

        let (pfd_log, cfd_log) = unistd::pipe().chain_err(|| "failed to create pipe")?;
        fcntl::fcntl(pfd_log, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

        let child_logger = logger.new(o!("action" => "child process log"));
        let log_handler = thread::spawn(move || {
            let log_file = unsafe { std::fs::File::from_raw_fd(pfd_log) };
            let mut reader = BufReader::new(log_file);

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Err(e) => {
                        info!(child_logger, "read child process log error: {:?}", e);
                        break;
                    }
                    Ok(count) => {
                        if count == 0 {
                            info!(child_logger, "read child process log end",);
                            break;
                        }

                        info!(child_logger, "{}", line);
                    }
                }
            }
        });

        info!(logger, "exec fifo opened!");
        let (prfd, cwfd) = unistd::pipe().chain_err(|| "failed to create pipe")?;
        let (crfd, pwfd) = unistd::pipe().chain_err(|| "failed to create pipe")?;
        fcntl::fcntl(prfd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
        fcntl::fcntl(pwfd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

        defer!({
            unistd::close(prfd);
            unistd::close(pwfd);
        });

        let mut child_stdin = std::process::Stdio::null();
        let mut child_stdout = std::process::Stdio::null();
        let mut child_stderr = std::process::Stdio::null();
        let mut stdin = -1;
        let mut stdout = -1;
        let mut stderr = -1;

        if tty {
            let pseduo = pty::openpty(None, None)?;
            p.term_master = Some(pseduo.master);
            fcntl::fcntl(pseduo.master, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));
            fcntl::fcntl(pseduo.slave, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC));

            child_stdin = unsafe { std::process::Stdio::from_raw_fd(pseduo.slave) };
            child_stdout = unsafe { std::process::Stdio::from_raw_fd(pseduo.slave) };
            child_stderr = unsafe { std::process::Stdio::from_raw_fd(pseduo.slave) };
        } else {
            stdin = p.stdin.unwrap();
            stdout = p.stdout.unwrap();
            stderr = p.stderr.unwrap();
            child_stdin = unsafe { std::process::Stdio::from_raw_fd(stdin) };
            child_stdout = unsafe { std::process::Stdio::from_raw_fd(stdout) };
            child_stderr = unsafe { std::process::Stdio::from_raw_fd(stderr) };
        }

        let old_pid_ns = match fcntl::open(PID_NS_PATH, OFlag::O_CLOEXEC, Mode::empty()) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    logger,
                    "cannot open pid ns path: {} with error: {:?}", PID_NS_PATH, e
                );
                return Err(e.into());
            }
        };

        //restore the parent's process's pid namespace.
        defer!({
            sched::setns(old_pid_ns, CloneFlags::CLONE_NEWPID);
            unistd::close(old_pid_ns);
        });

        let mut pidns = None;
        if !p.init {
            pidns = Some(get_pid_namespace(&self.logger, linux)?);
        }

        if pidns.is_some() {
            sched::setns(pidns.unwrap(), CloneFlags::CLONE_NEWPID)
                .chain_err(|| "failed to join pidns")?;
            unistd::close(pidns.unwrap())?;
        } else {
            sched::unshare(CloneFlags::CLONE_NEWPID)?;
        }

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

        let mut child = child.spawn()?;

        unistd::close(crfd)?;
        unistd::close(cwfd)?;
        unistd::close(cfd_log)?;

        p.pid = child.id() as i32;
        if p.init {
            self.init_process_pid = p.pid;
        }

        if p.init {
            unistd::close(fifofd);
        }

        info!(logger, "child pid: {}", p.pid);

        match join_namespaces(
            &logger,
            &spec,
            &p,
            self.cgroup_manager.as_ref().unwrap(),
            &st,
            &mut child,
            pwfd,
            prfd,
        ) {
            Ok(_) => (),
            Err(e) => {
                error!(logger, "create container process error {:?}", e);
                // kill the child process.
                signal::kill(Pid::from_raw(p.pid), Some(Signal::SIGKILL));
                return Err(e);
            }
        };

        info!(logger, "entered namespaces!");

        self.status = Some("created".to_string());
        self.created = SystemTime::now();

        // create the pipes for notify process exited
        let (exit_pipe_r, exit_pipe_w) = unistd::pipe2(OFlag::O_CLOEXEC)
            .chain_err(|| "failed to create pipe")
            .map_err(|e| {
                signal::kill(Pid::from_raw(child.id() as i32), Some(Signal::SIGKILL));
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
        log_handler.join();
        info!(logger, "create process completed");
        return Ok(());
    }

    fn run(&mut self, p: Process) -> Result<()> {
        let init = p.init;
        self.start(p)?;

        if init {
            self.exec()?;
            self.status = Some("running".to_string());
        }

        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        let spec = self.config.spec.as_ref().unwrap();
        let st = self.oci_state()?;

        for pid in self.processes.keys() {
            signal::kill(Pid::from_raw(*pid), Some(Signal::SIGKILL))?;
        }

        if spec.hooks.is_some() {
            info!(self.logger, "poststop");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.poststop.iter() {
                execute_hook(&self.logger, h, &st)?;
            }
        }

        self.status = Some("stopped".to_string());
        fs::remove_dir_all(&self.root)?;
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

        self.status = Some("running".to_string());
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

fn do_exec(args: &[String]) -> Result<()> {
    let path = &args[0];
    let p = CString::new(path.to_string()).unwrap();
    let sa: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();
    let a: Vec<&CStr> = sa.iter().map(|s| s.as_c_str()).collect();

    if let Err(e) = unistd::execvp(p.as_c_str(), a.as_slice()) {
        //     info!(logger, "execve failed!!!");
        //     info!(logger, "binary: {:?}, args: {:?}, envs: {:?}", p, a, env);
        match e {
            nix::Error::Sys(errno) => {
                std::process::exit(errno as i32);
            }
            _ => std::process::exit(-2),
        }
    }
    // should never reach here
    Ok(())
}

fn update_namespaces(logger: &Logger, spec: &mut Spec, init_pid: RawFd) -> Result<()> {
    let linux = match spec.linux.as_mut() {
        None => {
            return Err(
                ErrorKind::ErrorCode("Spec didn't container linux field".to_string()).into(),
            )
        }
        Some(l) => l,
    };

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

fn get_pid_namespace(logger: &Logger, linux: &Linux) -> Result<RawFd> {
    for ns in &linux.namespaces {
        if ns.r#type == "pid" {
            if ns.path == "" {
                error!(logger, "pid ns path is empty");
                return Err(ErrorKind::ErrorCode("pid ns path is empty".to_string()).into());
            }

            let fd = match fcntl::open(ns.path.as_str(), OFlag::O_CLOEXEC, Mode::empty()) {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        logger,
                        "cannot open type: {} path: {}",
                        ns.r#type.clone(),
                        ns.path.clone()
                    );
                    error!(logger, "error is : {}", e.as_errno().unwrap().desc());
                    return Err(e.into());
                }
            };

            return Ok(fd);
        }
    }

    Err(ErrorKind::ErrorCode("cannot find the pid ns".to_string()).into())
}

fn is_userns_enabled(linux: &Linux) -> bool {
    for ns in &linux.namespaces {
        if ns.r#type == "user" && ns.path == "" {
            return true;
        }
    }

    false
}

fn get_namespaces(linux: &Linux) -> Result<Vec<LinuxNamespace>> {
    let mut ns: Vec<LinuxNamespace> = Vec::new();
    for i in &linux.namespaces {
        ns.push(LinuxNamespace {
            r#type: i.r#type.clone(),
            path: i.path.clone(),
        });
    }
    Ok(ns)
}

fn join_namespaces(
    logger: &Logger,
    spec: &Spec,
    p: &Process,
    cm: &FsManager,
    st: &OCIState,
    child: &mut Child,
    pwfd: RawFd,
    prfd: RawFd,
) -> Result<()> {
    let logger = logger.new(o!("action" => "join-namespaces"));

    let linux = spec.linux.as_ref().unwrap();
    let res = linux.resources.as_ref();

    let userns = is_userns_enabled(linux);

    info!(logger, "try to send spec from parent to child");
    let spec_str = serde_json::to_string(spec)?;
    write_sync(pwfd, SYNC_DATA, spec_str.as_str())?;

    info!(logger, "wait child received oci spec");

    //    child.try_wait()?;
    read_sync(prfd)?;

    info!(logger, "send oci process from parent to child");
    let process_str = serde_json::to_string(&p.oci)?;
    write_sync(pwfd, SYNC_DATA, process_str.as_str())?;

    info!(logger, "wait child received oci process");
    read_sync(prfd)?;

    let cm_str = serde_json::to_string(cm)?;
    write_sync(pwfd, SYNC_DATA, cm_str.as_str())?;

    //wait child setup user namespace
    info!(logger, "wait child setup user namespace");
    read_sync(prfd)?;

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
    if p.init {
        if res.is_some() {
            info!(logger, "apply cgroups!");
            cm.set(res.unwrap(), false)?;
        }
    }

    if res.is_some() {
        cm.apply(p.pid)?;
    }

    info!(logger, "notify child to continue");
    // notify child to continue
    write_sync(pwfd, SYNC_SUCCESS, "")?;

    if p.init {
        info!(logger, "notify child parent ready to run prestart hook!");
        let _ = read_sync(prfd)?;

        info!(logger, "get ready to run prestart hook!");

        // run prestart hook
        if spec.hooks.is_some() {
            info!(logger, "prestart hook");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.prestart.iter() {
                execute_hook(&logger, h, st)?;
            }
        }

        // notify child run prestart hooks completed
        info!(logger, "notify child run prestart hook completed!");
        write_sync(pwfd, SYNC_SUCCESS, "")?;

        info!(logger, "notify child parent ready to run poststart hook!");
        // wait to run poststart hook
        read_sync(prfd)?;
        info!(logger, "get ready to run poststart hook!");

        //run poststart hook
        if spec.hooks.is_some() {
            info!(logger, "poststart hook");
            let hooks = spec.hooks.as_ref().unwrap();
            for h in hooks.poststart.iter() {
                execute_hook(&logger, h, st)?;
            }
        }
    }

    info!(logger, "wait for child process ready to run exec");
    read_sync(prfd)?;

    Ok(())
}

fn write_mappings(logger: &Logger, path: &str, maps: &[LinuxIDMapping]) -> Result<()> {
    let mut data = String::new();
    for m in maps {
        if m.size == 0 {
            continue;
        }

        let val = format!("{} {} {}\n", m.container_id, m.host_id, m.size);
        data = data + &val;
    }

    info!(logger, "mapping: {}", data);
    if !data.is_empty() {
        let fd = fcntl::open(path, OFlag::O_WRONLY, Mode::empty())?;
        defer!(unistd::close(fd).unwrap());
        match unistd::write(fd, data.as_bytes()) {
            Ok(_) => {}
            Err(e) => {
                info!(logger, "cannot write mapping");
                return Err(e.into());
            }
        }
    }
    Ok(())
}

fn setid(uid: Uid, gid: Gid) -> Result<()> {
    // set uid/gid
    if let Err(e) = prctl::set_keep_capabilities(true) {
        bail!(format!("set keep capabilities returned {}", e));
    };
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

    if let Err(e) = prctl::set_keep_capabilities(false) {
        bail!(format!("set keep capabilities returned {}", e));
    };
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

        if let Err(e) = fs::create_dir_all(root.as_str()) {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(e).chain_err(|| format!("container {} already exists", id.as_str()));
            }

            return Err(e).chain_err(|| format!("fail to create container directory {}", root));
        }

        unistd::chown(
            root.as_str(),
            Some(unistd::getuid()),
            Some(unistd::getgid()),
        )
        .chain_err(|| format!("cannot change onwer of container {} root", id))?;

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

        Ok(LinuxContainer {
            id: id.clone(),
            root,
            cgroup_manager: Some(cgroup_manager),
            status: Some("stopped".to_string()),
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
        Err(ErrorKind::ErrorCode("not supported".to_string()).into())
    }
    /*
        fn new_parent_process(&self, p: &Process) -> Result<Box<ParentProcess>> {
            let (pfd, cfd) = socket::socketpair(AddressFamily::Unix,
                            SockType::Stream, SockProtocol::Tcp,
                            SockFlag::SOCK_CLOEXEC)?;

            let cmd = Command::new(self.init_path)
                            .args(self.init_args[1..])
                            .env("_LIBCONTAINER_INITPIPE", format!("{}",
                                    cfd))
                            .env("_LIBCONTAINER_STATEDIR", self.root)
                            .current_dir(Path::new(self.config.rootfs))
                            .stdin(p.stdin)
                            .stdout(p.stdout)
                            .stderr(p.stderr);

            if p.console_socket.is_some() {
                cmd.env("_LIBCONTAINER_CONSOLE", format!("{}",
                        unsafe { p.console_socket.unwrap().as_raw_fd() }));
            }

            if !p.init {
                return self.new_setns_process(p, cmd, pfd, cfd);
            }

            let fifo_file = format!("{}/{}", self.root, EXEC_FIFO_FILENAME);
            let fifofd = fcntl::open(fifo_file,
                    OFlag::O_PATH | OFlag::O_CLOEXEC,
                    Mode::from_bits(0).unwrap())?;

            cmd.env("_LIBCONTAINER_FIFOFD", format!("{}", fifofd));

            self.new_init_process(p, cmd, pfd, cfd)
        }

        fn new_setns_process(&self, p: &Process, cmd: &mut Command, pfd: Rawfd, cfd: Rawfd) -> Result<SetnsProcess> {
        }

        fn new_init_process(&self, p: &Process, cmd: &mut Command, pfd: Rawfd, cfd: Rawfd) -> Result<InitProcess> {
            cmd.env("_LINCONTAINER_INITTYPE", INITSTANDARD);
        }
    */
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

use std::error::Error as StdError;
use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

fn execute_hook(logger: &Logger, h: &Hook, st: &OCIState) -> Result<()> {
    let logger = logger.new(o!("action" => "execute-hook"));

    let binary = PathBuf::from(h.path.as_str());
    let path = binary.canonicalize()?;
    if !path.exists() {
        return Err(ErrorKind::Nix(Error::from_errno(Errno::EINVAL)).into());
    }

    let args = h.args.clone();
    let envs = h.env.clone();
    let state = serde_json::to_string(st)?;
    //	state.push_str("\n");

    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
    match unistd::fork()? {
        ForkResult::Parent { child: _ch } => {
            let buf = read_sync(rfd)?;
            let buf_array: [u8; 4] = [buf[0], buf[1], buf[2], buf[3]];
            let status: i32 = i32::from_be_bytes(buf_array);

            info!(logger, "hook child: {}", _ch);

            // let _ = wait::waitpid(_ch,
            //	Some(WaitPidFlag::WEXITED | WaitPidFlag::__WALL));

            if status != 0 {
                if status == -libc::ETIMEDOUT {
                    return Err(ErrorKind::Nix(Error::from_errno(Errno::ETIMEDOUT)).into());
                } else if status == -libc::EPIPE {
                    return Err(ErrorKind::Nix(Error::from_errno(Errno::EPIPE)).into());
                } else {
                    return Err(ErrorKind::Nix(Error::from_errno(Errno::UnknownErrno)).into());
                }
            }

            return Ok(());
        }

        ForkResult::Child => {
            let (tx, rx) = mpsc::channel();
            let (tx_logger, rx_logger) = mpsc::channel();

            tx_logger.send(logger.clone()).unwrap();

            let handle = thread::spawn(move || {
                let logger = rx_logger.recv().unwrap();

                // write oci state to child
                let env: HashMap<String, String> = envs
                    .iter()
                    .map(|e| {
                        let v: Vec<&str> = e.split('=').collect();
                        (v[0].to_string(), v[1].to_string())
                    })
                    .collect();

                let mut child = Command::new(path.to_str().unwrap())
                    .args(args.iter())
                    .envs(env.iter())
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();

                //send out our pid
                tx.send(child.id() as libc::pid_t).unwrap();
                info!(logger, "hook grand: {}", child.id());

                child
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(state.as_bytes())
                    .unwrap();

                // read something from stdout for debug
                let mut out = String::new();
                child
                    .stdout
                    .as_mut()
                    .unwrap()
                    .read_to_string(&mut out)
                    .unwrap();
                info!(logger, "{}", out.as_str());
                match child.wait() {
                    Ok(exit) => {
                        let code: i32 = if exit.success() {
                            0
                        } else {
                            match exit.code() {
                                Some(c) => (c as u32 | 0x80000000) as i32,
                                None => exit.signal().unwrap(),
                            }
                        };

                        tx.send(code).unwrap();
                    }

                    Err(e) => {
                        info!(
                            logger,
                            "wait child error: {} {}",
                            e.description(),
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

                        tx.send(0).unwrap();
                    }
                }
            });

            let pid = rx.recv().unwrap();
            info!(logger, "hook grand: {}", pid);

            let status = {
                if let Some(timeout) = h.timeout {
                    match rx.recv_timeout(Duration::from_secs(timeout as u64)) {
                        Ok(s) => s,
                        Err(e) => {
                            let error = if e == RecvTimeoutError::Timeout {
                                -libc::ETIMEDOUT
                            } else {
                                -libc::EPIPE
                            };
                            let _ = signal::kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                            error
                        }
                    }
                } else {
                    if let Ok(s) = rx.recv() {
                        s
                    } else {
                        let _ = signal::kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                        -libc::EPIPE
                    }
                }
            };

            handle.join().unwrap();
            let _ = write_sync(wfd, status, "");
            // let _ = wait::waitpid(Pid::from_raw(pid),
            //	Some(WaitPidFlag::WEXITED | WaitPidFlag::__WALL));
            std::process::exit(0);
        }
    }
}
