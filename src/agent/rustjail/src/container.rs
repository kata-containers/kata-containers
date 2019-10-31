// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use lazy_static;
use protocols::oci::{Hook, Linux, LinuxNamespace, LinuxResources, POSIXRlimit, Spec};
use serde_json;
use std::ffi::CString;
use std::fs;
use std::mem;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
// use crate::sync::Cond;
use libc::pid_t;
use protocols::oci::{LinuxDevice, LinuxIDMapping};
use std::clone::Clone;
use std::fmt::Display;
use std::process::Command;

// use crate::configs::namespaces::{NamespaceType};
use crate::cgroups::Manager as CgroupManager;
use crate::process::Process;
// use crate::intelrdt::Manager as RdtManager;
use crate::errors::*;
use crate::specconv::CreateOpts;
// use crate::stats::Stats;
use crate::capabilities::{self, CAPSMAP};
use crate::cgroups::fs::{self as fscgroup, Manager as FsManager};
use crate::{mount, validator};

use protocols::agent::StatsContainerResponse;

use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
use nix::pty;
use nix::sched::{self, CloneFlags};
use nix::sys::signal::{self, Signal};
use nix::sys::socket::{self, ControlMessage, ControlMessageOwned, MsgFlags};
use nix::sys::stat::{self, Mode};
use nix::sys::uio::IoVec;
use nix::unistd::{self, ForkResult, Gid, Pid, Uid};
use nix::Error;

use libc;
use protobuf::{CachedSize, SingularPtrField, UnknownFields};

use oci::State as OCIState;
use std::collections::HashMap;

use slog::{debug, info, o, Logger};

const STATE_FILENAME: &'static str = "state.json";
const EXEC_FIFO_FILENAME: &'static str = "exec.fifo";
const VER_MARKER: &'static str = "1.2.5";

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
            Path: "/dev/null".to_string(),
            Type: "c".to_string(),
            Major: 1,
            Minor: 3,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
        v.push(LinuxDevice {
            Path: "/dev/zero".to_string(),
            Type: "c".to_string(),
            Major: 1,
            Minor: 5,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
        v.push(LinuxDevice {
            Path: "/dev/full".to_string(),
            Type: String::from("c"),
            Major: 1,
            Minor: 7,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
        v.push(LinuxDevice {
            Path: "/dev/tty".to_string(),
            Type: "c".to_string(),
            Major: 5,
            Minor: 0,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
        v.push(LinuxDevice {
            Path: "/dev/urandom".to_string(),
            Type: "c".to_string(),
            Major: 1,
            Minor: 9,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
        v.push(LinuxDevice {
            Path: "/dev/random".to_string(),
            Type: "c".to_string(),
            Major: 1,
            Minor: 8,
            FileMode: 0o066,
            UID: 0xffffffff,
            GID: 0xffffffff,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
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

        let root = oci.Root.as_ref().unwrap().Path.as_str();
        let path = fs::canonicalize(root)?;
        let bundle = path.parent().unwrap().to_str().unwrap().to_string();
        Ok(OCIState {
            version: oci.Version.clone(),
            id: self.id(),
            status,
            pid,
            bundle,
            annotations: oci.Annotations.clone(),
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
            .Linux
            .as_mut()
            .unwrap()
            .Resources = SingularPtrField::some(r);
        Ok(())
    }

    fn start(&mut self, mut p: Process) -> Result<()> {
        let fifo_file = format!("{}/{}", &self.root, EXEC_FIFO_FILENAME);
        info!(self.logger, "enter container.start!");
        let mut fifofd: RawFd = -1;
        if p.init {
            if let Ok(_) = stat::stat(fifo_file.as_str()) {
                return Err(ErrorKind::ErrorCode("exec fifo exists".to_string()).into());
            }
            unistd::mkfifo(fifo_file.as_str(), Mode::from_bits(0o622).unwrap())?;
            // defer!(fs::remove_file(&fifo_file)?);

            fifofd = fcntl::open(
                fifo_file.as_str(),
                OFlag::O_PATH | OFlag::O_CLOEXEC,
                Mode::from_bits(0).unwrap(),
            )?;
        }
        info!(self.logger, "exec fifo opened!");

        lazy_static::initialize(&NAMESPACES);
        lazy_static::initialize(&DEFAULT_DEVICES);
        lazy_static::initialize(&RLIMITMAPS);
        lazy_static::initialize(&CAPSMAP);
        fscgroup::init_static();

        if self.config.spec.is_none() {
            return Err(ErrorKind::ErrorCode("no spec".to_string()).into());
        }

        let spec = self.config.spec.as_ref().unwrap();
        if spec.Linux.is_none() {
            return Err(ErrorKind::ErrorCode("no linux config".to_string()).into());
        }

        let linux = spec.Linux.as_ref().unwrap();
        // get namespace vector to join/new
        let nses = get_namespaces(&linux, p.init, self.init_process_pid)?;
        info!(self.logger, "got namespaces {:?}!\n", nses);
        let mut to_new = CloneFlags::empty();
        let mut to_join = Vec::new();
        let mut pidns = false;
        let mut userns = false;
        for ns in &nses {
            let s = NAMESPACES.get(&ns.Type.as_str());
            if s.is_none() {
                return Err(ErrorKind::ErrorCode("invalid ns type".to_string()).into());
            }
            let s = s.unwrap();

            if ns.Path.is_empty() {
                to_new.set(*s, true);
            } else {
                let fd = match fcntl::open(ns.Path.as_str(), OFlag::empty(), Mode::empty()) {
                    Ok(v) => v,
                    Err(e) => {
                        info!(
                            self.logger,
                            "cannot open type: {} path: {}",
                            ns.Type.clone(),
                            ns.Path.clone()
                        );
                        info!(self.logger, "error is : {}", e.as_errno().unwrap().desc());
                        return Err(e.into());
                    }
                };
                //		.chain_err(|| format!("fail to open ns {}", &ns.Type))?;
                to_join.push((*s, fd));
            }

            if *s == CloneFlags::CLONE_NEWPID {
                pidns = true;
            }
        }

        if to_new.contains(CloneFlags::CLONE_NEWUSER) {
            userns = true;
        }

        let mut parent: u32 = 0;
        let st = self.oci_state()?;

        let (child, cfd) = match join_namespaces(
            &self.logger,
            &spec,
            to_new,
            &to_join,
            pidns,
            userns,
            p.init,
            self.config.no_pivot_root,
            self.cgroup_manager.as_ref().unwrap(),
            &st,
            &mut parent,
        ) {
            Ok((u, v)) => (u, v),
            Err(e) => {
                if parent == 0 {
                    info!(self.logger, "parent process error out!");
                    return Err(e);
                } else if parent == 1 {
                    info!(self.logger, "child process 1 error out!");
                    std::process::exit(-1);
                } else {
                    info!(self.logger, "child process 2 error out!");
                    std::process::exit(-2);
                }
            }
        };
        info!(self.logger, "entered namespaces!");
        if child != Pid::from_raw(-1) {
            // parent
            p.pid = child.as_raw();
            self.status = Some("created".to_string());
            if p.init {
                self.init_process_pid = p.pid;
                unistd::close(fifofd)?;
            }
            self.created = SystemTime::now();
            // defer!({ self.processes.insert(p.pid, p); () });
            // parent process need to receive ptmx masterfd
            // and set it up in process struct

            unistd::close(p.stdin.unwrap())?;
            unistd::close(p.stderr.unwrap())?;
            unistd::close(p.stdout.unwrap())?;

            for &(_, fd) in &to_join {
                let _ = unistd::close(fd);
            }

            // create the pipes for notify process exited
            let (exit_pipe_r, exit_pipe_w) =
                unistd::pipe2(OFlag::O_CLOEXEC).chain_err(|| "failed to create pipe")?;
            p.exit_pipe_w = Some(exit_pipe_w);
            p.exit_pipe_r = Some(exit_pipe_r);

            let console_fd = if p.parent_console_socket.is_some() {
                p.parent_console_socket.unwrap()
            } else {
                self.processes.insert(p.pid, p);
                return Ok(());
            };

            let mut v: Vec<u8> = vec![0; 40];
            let iov = IoVec::from_mut_slice(v.as_mut_slice());
            let mut c: Vec<u8> = vec![0; 40];

            match socket::recvmsg(console_fd, &[iov], Some(&mut c), MsgFlags::empty()) {
                Ok(rmsg) => {
                    let cmsg: Vec<ControlMessageOwned> = rmsg.cmsgs().collect();
                    // expect the vector lenght 1
                    if cmsg.len() != 1 {
                        return Err(
                            ErrorKind::ErrorCode("error in semd/recvmsg!".to_string()).into()
                        );
                    }

                    match &cmsg[0] {
                        ControlMessageOwned::ScmRights(v) => {
                            if v.len() != 1 {
                                return Err(ErrorKind::ErrorCode(
                                    "error in send/recvmsg!".to_string(),
                                )
                                .into());
                            }

                            p.term_master = Some(v[0]);
                        }
                        // all other cases are error
                        _ => {
                            return Err(
                                ErrorKind::ErrorCode("error in send/recvmsg!".to_string()).into()
                            );
                        }
                    }
                }
                Err(e) => return Err(ErrorKind::Nix(e).into()),
            }

            unistd::close(p.parent_console_socket.unwrap())?;
            unistd::close(p.console_socket.unwrap())?;

            // turn off echo
            // let mut term = termios::tcgetattr(p.term_master.unwrap())?;
            // term.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
            // termios::tcsetattr(p.term_master.unwrap(), SetArg::TCSANOW, &term)?;

            self.processes.insert(p.pid, p);

            return Ok(());
        } // end parent

        // setup stdio in child process
        // need fd to send master fd to parent... store the fd in
        // process struct?
        setup_stdio(&p)?;

        if to_new.contains(CloneFlags::CLONE_NEWNS) {
            info!(self.logger, "finish rootfs!");
            mount::finish_rootfs(spec)?;
        }

        if !p.oci.Cwd.is_empty() {
            debug!(self.logger, "cwd: {}", p.oci.Cwd.as_str());
            unistd::chdir(p.oci.Cwd.as_str())?;
        }

        // setup uid/gid
        info!(self.logger, "{:?}", p.oci.clone());

        if p.oci.User.is_some() {
            let guser = p.oci.User.as_ref().unwrap();

            let uid = Uid::from_raw(guser.UID);
            let gid = Gid::from_raw(guser.GID);

            setid(uid, gid)?;

            if !guser.AdditionalGids.is_empty() {
                setgroups(guser.AdditionalGids.as_slice())?;
            }
        }

        // NoNewPeiviledges, Drop capabilities
        if p.oci.NoNewPrivileges {
            if let Err(_) = prctl::set_no_new_privileges(true) {
                return Err(
                    ErrorKind::ErrorCode("cannot set no new privileges".to_string()).into(),
                );
            }
        }

        if p.oci.Capabilities.is_some() {
            let c = p.oci.Capabilities.as_ref().unwrap();
            info!(self.logger, "drop capabilities!");
            capabilities::drop_priviledges(&self.logger, c)?;
        }

        if p.init {
            // notify parent to run poststart hooks
            // cfd is closed when return from join_namespaces
            // should retunr cfile instead of cfd?
            write_sync(cfd, 0)?;
        }

        // new and the stat parent process
        // For init process, we need to setup a lot of things
        // For exec process, only need to join existing namespaces,
        // the namespaces are got from init process or from
        // saved spec.
        debug!(self.logger, "before setup execfifo!");
        info!(self.logger, "{}", VER_MARKER);
        if p.init {
            let fd = fcntl::open(
                format!("/proc/self/fd/{}", fifofd).as_str(),
                OFlag::O_RDONLY | OFlag::O_CLOEXEC,
                Mode::from_bits_truncate(0),
            )?;
            unistd::close(fifofd)?;
            let mut buf: &mut [u8] = &mut [0];
            unistd::read(fd, &mut buf)?;
        }

        // exec process
        let args = p.oci.Args.to_vec();
        let env = p.oci.Env.to_vec();
        do_exec(&self.logger, &args[0], &args, &env)?;

        Err(ErrorKind::ErrorCode("fail to create container".to_string()).into())
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

        if spec.Hooks.is_some() {
            info!(self.logger, "poststop");
            let hooks = spec.Hooks.as_ref().unwrap();
            for h in hooks.Poststop.iter() {
                execute_hook(&self.logger, h, &st)?;
            }
        }

        self.status = Some("stopped".to_string());
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
        info!(self.logger, "container {} stared", &self.id);
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

fn do_exec(logger: &Logger, path: &str, args: &[String], env: &[String]) -> Result<()> {
    let logger = logger.new(o!("command" => "exec"));

    let p = CString::new(path.to_string()).unwrap();
    let a: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();

    for (key, _) in env::vars() {
        env::remove_var(key);
    }

    for e in env.iter() {
        let v: Vec<&str> = e.split("=").collect();
        if v.len() != 2 {
            info!(logger, "incorrect env config!");
        }
        env::set_var(v[0], v[1]);
    }
    /*
    let env: Vec<CString> = env
        .iter()
        .map(|s| CString::new(s.to_string()).unwrap_or_default())
        .collect();
        */
    // execvp doesn't use env for the search path, so we set env manually
    debug!(logger, "exec process right now!");
    if let Err(e) = unistd::execvp(&p, &a) {
        info!(logger, "execve failed!!!");
        info!(logger, "binary: {:?}, args: {:?}, envs: {:?}", p, a, env);
        match e {
            nix::Error::Sys(errno) => {
                info!(logger, "{}", errno.desc());
            }
            Error::InvalidPath => {
                info!(logger, "invalid path");
            }
            Error::InvalidUtf8 => {
                info!(logger, "invalid utf8");
            }
            Error::UnsupportedOperation => {
                info!(logger, "unsupported operation");
            }
        }
        std::process::exit(-2);
    }
    // should never reach here
    Ok(())
}

fn get_namespaces(linux: &Linux, init: bool, init_pid: pid_t) -> Result<Vec<LinuxNamespace>> {
    let mut ns: Vec<LinuxNamespace> = Vec::new();
    if init {
        for i in &linux.Namespaces {
            ns.push(LinuxNamespace {
                Type: i.Type.clone(),
                Path: i.Path.clone(),
                unknown_fields: UnknownFields::default(),
                cached_size: CachedSize::default(),
            });
        }
    } else {
        for i in NAMESPACES.keys() {
            let ns_path = format!("/proc/{}/ns/{}", init_pid, TYPETONAME.get(i).unwrap());
            let ns_path_buf = Path::new(&ns_path).read_link()?;

            let init_ns_path = format!(
                "/proc/{}/ns/{}",
                unistd::getpid(),
                TYPETONAME.get(i).unwrap()
            );
            let init_ns_path_buf = Path::new(&init_ns_path).read_link()?;

            // ignore the namespace which is the same with system init namespace,
            // since it shouldn't be join.
            if !ns_path_buf.eq(&init_ns_path_buf) {
                ns.push(LinuxNamespace {
                    Type: i.to_string(),
                    Path: ns_path,
                    unknown_fields: UnknownFields::default(),
                    cached_size: CachedSize::default(),
                });
            }
        }
    }
    Ok(ns)
}

pub const PIDSIZE: usize = mem::size_of::<pid_t>();

fn read_sync(fd: RawFd) -> Result<pid_t> {
    let mut v: [u8; PIDSIZE] = [0; PIDSIZE];
    let mut len = 0;

    loop {
        match unistd::read(fd, &mut v[len..]) {
            Ok(l) => {
                len += l;
                if len == PIDSIZE {
                    break;
                }
            }

            Err(e) => {
                if e != Error::from_errno(Errno::EINTR) {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(pid_t::from_be_bytes(v))
}

fn write_sync(fd: RawFd, pid: pid_t) -> Result<()> {
    let buf = pid.to_be_bytes();
    let mut len = 0;

    loop {
        match unistd::write(fd, &buf[len..]) {
            Ok(l) => {
                len += l;
                if len == PIDSIZE {
                    break;
                }
            }

            Err(e) => {
                if e != Error::from_errno(Errno::EINTR) {
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}

fn join_namespaces(
    logger: &Logger,
    spec: &Spec,
    to_new: CloneFlags,
    to_join: &Vec<(CloneFlags, RawFd)>,
    pidns: bool,
    userns: bool,
    init: bool,
    no_pivot: bool,
    cm: &FsManager,
    st: &OCIState,
    parent: &mut u32,
) -> Result<(Pid, RawFd)> {
    let logger = logger.new(o!("action" => "join-namespaces"));

    // let ccond = Cond::new().chain_err(|| "create cond failed")?;
    // let pcond = Cond::new().chain_err(|| "create cond failed")?;
    let (pfd, cfd) = unistd::pipe2(OFlag::O_CLOEXEC).chain_err(|| "failed to create pipe")?;
    let (crfd, pwfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;

    let linux = spec.Linux.as_ref().unwrap();
    let res = linux.Resources.as_ref();

    match unistd::fork()? {
        ForkResult::Parent { child } => {
            // let mut pfile = unsafe { File::from_raw_fd(pfd) };
            unistd::close(cfd)?;
            unistd::close(crfd)?;

            //wait child setup user namespace
            let _ = read_sync(pfd)?;

            if userns {
                // setup uid/gid mappings
                write_mappings(
                    &logger,
                    &format!("/proc/{}/uid_map", child.as_raw()),
                    &linux.UIDMappings,
                )?;
                write_mappings(
                    &logger,
                    &format!("/proc/{}/gid_map", child.as_raw()),
                    &linux.GIDMappings,
                )?;
            }

            // apply cgroups
            if init {
                if res.is_some() {
                    info!(logger, "apply cgroups!");
                    cm.set(res.unwrap(), false)?;
                }
            }

            if res.is_some() {
                cm.apply(child.as_raw())?;
            }

            write_sync(pwfd, 0)?;

            let mut pid = child.as_raw();
            info!(logger, "first child! {}", pid);
            info!(logger, "wait for final child!");
            if pidns {
                pid = read_sync(pfd)?;
                // pfile.read_to_string(&mut json)?;
                /*
                let msg: SyncPC = match serde_json::from_reader(&mut pfile) {
                    Ok(u) => u,
                    Err(e) => {
                        match e.classify() {
                            Category::Io => info!("Io error!"),
                            Category::Syntax => info!("syntax error!"),
                            Category::Data => info!("data error!"),
                            Category::Eof => info!("end of file!"),
                        }

                        return Err(ErrorKind::Serde(e).into());
                    }
                };
                */
                // notify child continue
                info!(logger, "got final child pid! {}", pid);
                write_sync(pwfd, 0)?;
                info!(logger, "resume child!");
                // wait for child to exit
                // Since the child would be reaped by our reaper, so
                // there is no need reap the child here.
                // wait::waitpid(Some(child), None);
            }
            // read out child pid here. we don't use
            // cgroup to get it
            // and the wait for child exit to get grandchild

            if init {
                info!(logger, "wait for hook!");
                let _ = read_sync(pfd)?;

                // run prestart hook
                if spec.Hooks.is_some() {
                    info!(logger, "prestart");
                    let hooks = spec.Hooks.as_ref().unwrap();
                    for h in hooks.Prestart.iter() {
                        execute_hook(&logger, h, st)?;
                    }
                }

                // notify child run prestart hooks completed
                write_sync(pwfd, 0)?;

                // wait to run poststart hook
                let _ = read_sync(pfd)?;
                //run poststart hook
                if spec.Hooks.is_some() {
                    info!(logger, "poststart");
                    let hooks = spec.Hooks.as_ref().unwrap();
                    for h in hooks.Poststart.iter() {
                        execute_hook(&logger, h, st)?;
                    }
                }
            }
            unistd::close(pfd)?;
            unistd::close(pwfd)?;

            return Ok((Pid::from_raw(pid), cfd));
        }
        ForkResult::Child => {
            *parent = 1;
            unistd::close(pfd)?;
            unistd::close(pwfd)?;
            // set oom_score_adj

            let p = if spec.Process.is_some() {
                spec.Process.as_ref().unwrap()
            } else {
                return Err(nix::Error::Sys(Errno::EINVAL).into());
            };

            if p.OOMScoreAdj > 0 {
                fs::write(
                    "/proc/self/oom_score_adj",
                    p.OOMScoreAdj.to_string().as_bytes(),
                )?
            }

            // set rlimit
            for rl in p.Rlimits.iter() {
                setrlimit(rl)?;
            }

            if userns {
                sched::unshare(CloneFlags::CLONE_NEWUSER)?;
            }

            write_sync(cfd, 0)?;
            let _ = read_sync(crfd)?;

            if userns {
                setid(Uid::from_raw(0), Gid::from_raw(0))?;
            }
        }
    }

    // child process continues
    // let mut cfile = unsafe { File::from_raw_fd(cfd) };
    let mut mount_fd = -1;
    let mut bind_device = false;
    for &(s, fd) in to_join {
        if s == CloneFlags::CLONE_NEWNS {
            mount_fd = fd;
            continue;
        }

        // just skip user namespace for now
        // we cannot join user namespace in multithreaded
        // program, which is us(kata-agent using grpc)
        // To fix this
        // 1. write kata-agent as singlethread program
        // 2. use a binary to exec OR self exec to enter
        //    namespaces before multithreaded, the way
        //    rustjail works
        /*
                if s == CloneFlags::CLONE_NEWUSER {
                    unistd::close(fd)?;
                    continue;
                }
        */
        if let Err(e) = sched::setns(fd, s) {
            info!(logger, "setns error: {}", e.as_errno().unwrap().desc());
            info!(logger, "setns: ns type: {:?}", s);
            if s == CloneFlags::CLONE_NEWUSER {
                if e.as_errno().unwrap() != Errno::EINVAL {
                    return Err(e.into());
                }
            } else {
                return Err(e.into());
            }
        }
        unistd::close(fd)?;

        if s == CloneFlags::CLONE_NEWUSER {
            setid(Uid::from_raw(0), Gid::from_raw(0))?;
            bind_device = true;
        }
    }

    info!(logger, "to_new: {:?}", to_new);
    sched::unshare(to_new & !CloneFlags::CLONE_NEWUSER)?;

    if userns {
        bind_device = true;
    }

    // create a pipe for sync between parent and child.
    // here we should make sure the parent return pid before
    // the child notify grand parent to run hooks, otherwise
    // both of the parent and his child would write cfd at the same
    // time which would mesh the grand parent to read.
    let (chfd, phfd) = unistd::pipe2(OFlag::O_CLOEXEC)
        .chain_err(|| "failed to create pipe for syncing run hooks")?;

    if pidns {
        match unistd::fork()? {
            ForkResult::Parent { child } => {
                unistd::close(chfd)?;
                // set child pid to topmost parent and the exit
                write_sync(cfd, child.as_raw())?;

                info!(
                    logger,
                    "json: {}",
                    serde_json::to_string(&SyncPC {
                        pid: child.as_raw()
                    })
                    .unwrap()
                );
                // wait for parent read it and the continue
                info!(logger, "after send out child pid!");
                let _ = read_sync(crfd)?;

                // notify child to continue.
                write_sync(phfd, 0)?;
                std::process::exit(0);
            }
            ForkResult::Child => {
                *parent = 2;
                unistd::close(phfd)?;
            }
        }
    }

    if to_new.contains(CloneFlags::CLONE_NEWUTS) {
        unistd::sethostname(&spec.Hostname)?;
    }

    let rootfs = spec.Root.as_ref().unwrap().Path.as_str();
    let root = fs::canonicalize(rootfs)?;
    let rootfs = root.to_str().unwrap();

    if to_new.contains(CloneFlags::CLONE_NEWNS) {
        // setup rootfs
        info!(logger, "setup rootfs!");
        mount::init_rootfs(&logger, &spec, &cm.paths, &cm.mounts, bind_device)?;
    }

    // wait until parent notified
    if pidns {
        let _ = read_sync(chfd)?;
    }
    unistd::close(chfd)?;

    if init {
        // notify parent to run prestart hooks
        write_sync(cfd, 0)?;
        // wait parent run prestart hooks
        let _ = read_sync(crfd)?;
    }

    unistd::close(crfd)?;

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
        set_sysctls(&linux.Sysctl)?;
        unistd::chdir("/")?;
        if let Err(_) = stat::stat("marker") {
            info!(logger, "not in expect root!!");
        }
        info!(logger, "in expect rootfs!");

        if let Err(_) = stat::stat("/bin/sh") {
            info!(logger, "no '/bin/sh'???");
        }
    }

    // notify parent to continue before block on exec fifo

    info!(logger, "rootfs: {}", &rootfs);

    // block on exec fifo

    Ok((Pid::from_raw(-1), cfd))
}

fn setup_stdio(p: &Process) -> Result<()> {
    if p.console_socket.is_some() {
        // we can setup ptmx master for process
        // turn off echo
        // let mut term = termios::tcgetattr(0)?;
        // termios::cfmakeraw(&mut term);
        // term.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
        // term.control_chars[VMIN] = 1;
        // term.control_chars[VTIME] = 0;

        let pseduo = pty::openpty(None, None)?;
        defer!(unistd::close(pseduo.master).unwrap());
        let data: &[u8] = b"/dev/ptmx";
        let iov = [IoVec::from_slice(&data)];
        let fds = [pseduo.master];
        let cmsg = ControlMessage::ScmRights(&fds);
        let mut console_fd = p.console_socket.unwrap();

        socket::sendmsg(console_fd, &iov, &[cmsg], MsgFlags::empty(), None)?;

        unistd::close(console_fd)?;
        unistd::close(p.parent_console_socket.unwrap())?;
        console_fd = pseduo.slave;

        unistd::setsid()?;
        unsafe {
            libc::ioctl(console_fd, libc::TIOCSCTTY);
        }
        unistd::dup2(console_fd, 0)?;
        unistd::dup2(console_fd, 1)?;
        unistd::dup2(console_fd, 2)?;

        // turn off echo
        // let mut term = termios::tcgetattr(0)?;
        // term.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
        // termios::tcsetattr(0, SetArg::TCSANOW, &term)?;

        if console_fd > 2 {
            unistd::close(console_fd)?;
        }
    } else {
        // dup stdin/stderr/stdout
        unistd::dup2(p.stdin.unwrap(), 0)?;
        unistd::dup2(p.stdout.unwrap(), 1)?;
        unistd::dup2(p.stderr.unwrap(), 2)?;

        if p.stdin.unwrap() > 2 {
            unistd::close(p.stdin.unwrap())?;
        }

        if p.stdout.unwrap() > 2 {
            unistd::close(p.stdout.unwrap())?;
        }
        if p.stderr.unwrap() > 2 {
            unistd::close(p.stderr.unwrap())?;
        }
    }

    unistd::close(p.parent_stdin.unwrap())?;
    unistd::close(p.parent_stdout.unwrap())?;
    unistd::close(p.parent_stderr.unwrap())?;

    Ok(())
}

fn write_mappings(logger: &Logger, path: &str, maps: &[LinuxIDMapping]) -> Result<()> {
    let mut data = String::new();
    for m in maps {
        if m.Size == 0 {
            continue;
        }

        let val = format!("{} {} {}\n", m.ContainerID, m.HostID, m.Size);
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

        if spec.Linux.is_none() {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }

        let linux = spec.Linux.as_ref().unwrap();

        let cpath = if linux.CgroupsPath.is_empty() {
            format!("/{}", id.as_str())
        } else {
            linux.CgroupsPath.clone()
        };

        let cgroup_manager = FsManager::new(cpath.as_str())?;

        Ok(LinuxContainer {
            id: id,
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
            logger: logger.new(o!("module" => "rustjail", "subsystem" => "container")),
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

lazy_static! {
    pub static ref RLIMITMAPS: HashMap<String, libc::c_int> = {
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
        rlim_cur: limit.Soft,
        rlim_max: limit.Hard,
    };

    let res = if RLIMITMAPS.get(limit.Type.as_str()).is_some() {
        *RLIMITMAPS.get(limit.Type.as_str()).unwrap()
    } else {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    };

    let ret = unsafe { libc::setrlimit(res as i32, &rl as *const libc::rlimit) };

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

    let binary = PathBuf::from(h.Path.as_str());
    let path = binary.canonicalize()?;
    if !path.exists() {
        return Err(ErrorKind::Nix(Error::from_errno(Errno::EINVAL)).into());
    }

    let args = h.Args.clone();
    let envs = h.Env.clone();
    let state = serde_json::to_string(st)?;
    //	state.push_str("\n");

    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;
    match unistd::fork()? {
        ForkResult::Parent { child: _ch } => {
            let status = read_sync(rfd)?;

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

            let status: i32 = if h.Timeout > 0 {
                match rx.recv_timeout(Duration::from_secs(h.Timeout as u64)) {
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
            };

            handle.join().unwrap();
            let _ = write_sync(wfd, status);
            // let _ = wait::waitpid(Pid::from_raw(pid),
            //	Some(WaitPidFlag::WEXITED | WaitPidFlag::__WALL));
            std::process::exit(0);
        }
    }
}
