# Supported features

This is an approximate list of all the files under the `/proc` mount, and an indication if that feature/file is supported by the `procfs` crate.  Help is needed to keep this file up-to-date, so please open an issue or pull request if you spot something that's not right.

* [ ] `/proc/[pid]`
  * [ ] `/proc/[pid]/attr`
    * [ ] `/proc/[pid]/attr/current`
    * [ ] `/proc/[pid]/attr/exec`
    * [ ] `/proc/[pid]/attr/fscreate`
    * [ ] `/proc/[pid]/attr/keycreate`
    * [ ] `/proc/[pid]/attr/prev`
    * [ ] `/proc/[pid]/attr/socketcreate`
  * [x] `/proc/[pid]/autogroup`
  * [x] `/proc/[pid]/auxv`
  * [x] `/proc/[pid]/cgroup`
  * [ ] `/proc/[pid]/clear_refs`
  * [x] `/proc/[pid]/cmdline`
  * [x] `/proc/[pid]/comm`
  * [x] `/proc/[pid]/coredump_filter`
  * [ ] `/proc/[pid]/cpuset`
  * [x] `/proc/[pid]/cwd`
  * [x] `/proc/[pid]/environ`
  * [x] `/proc/[pid]/exe`
  * [x] `/proc/[pid]/fd/`
  * [ ] `/proc/[pid]/fdinfo/`
  * [ ] `/proc/[pid]/gid_map`
  * [x] `/proc/[pid]/io`
  * [x] `/proc/[pid]/limits`
  * [ ] `/proc/[pid]/map_files/`
  * [x] `/proc/[pid]/maps`
  * [ ] `/proc/[pid]/mem`
  * [x] `/proc/[pid]/mountinfo`
  * [ ] `/proc/[pid]/mounts`
  * [x] `/proc/[pid]/mountstats`
  * [x] `/proc/[pid]/ns/`
  * [ ] `/proc/[pid]/numa_maps`
  * [ ] `/proc/[pid]/oom_adj`
  * [x] `/proc/[pid]/oom_score`
  * [ ] `/proc/[pid]/oom_score_adj`
  * [ ] `/proc/[pid]/pagemap`
  * [ ] `/proc/[pid]/personality`
  * [x] `/proc/[pid]/root`
  * [ ] `/proc/[pid]/seccomp`
  * [ ] `/proc/[pid]/setgroups`
  * [ ] `/proc/[pid]/sched_autogroup_enabled`
  * [x] `/proc/[pid]/smaps`
  * [ ] `/proc/[pid]/stack`
  * [x] `/proc/[pid]/stat`
  * [x] `/proc/[pid]/statm`
  * [x] `/proc/[pid]/status`
  * [ ] `/proc/[pid]/syscall`
  * [ ] `/proc/[pid]/task`
    * [x] `/proc/[pid]/task/[tid]/stat`
    * [x] `/proc/[pid]/task/[tid]/status`
    * [x] `/proc/[pid]/task/[tid]/io`
    * [ ] `/proc/[pid]/task/[tid]/children`
  * [ ] `/proc/[pid]/timers`
  * [ ] `/proc/[pid]/timerslack_ns`
  * [ ] `/proc/[pid]/uid_map`
  * [ ] `/proc/[pid]/gid_map`
  * [x] `/proc/[pid]/wchan`
* [ ] `/proc/apm`
* [ ] `/proc/buddyinfo`
* [ ] `/proc/bus`
  * [ ] `/proc/bus/pccard`
  * [ ] `/proc/bus/pccard/drivers`
  * [ ] `/proc/bus/pci`
  * [ ] `/proc/bus/pci/devices`
* [x] `/proc/cmdline`
* [ ] `/proc/config.gz`
* [ ] `/proc/crypto`
* [ ] `/proc/cpuinfo`
* [ ] `/proc/devices`
* [x] `/proc/diskstats`
* [ ] `/proc/dma`
* [ ] `/proc/driver`
* [ ] `/proc/execdomains`
* [ ] `/proc/fb`
* [ ] `/proc/filesystems`
* [ ] `/proc/fs`
* [ ] `/proc/ide`
* [ ] `/proc/interrupts`
* [ ] `/proc/iomem`
* [ ] `/proc/ioports`
* [ ] `/proc/kallsyms`
* [ ] `/proc/kcore`
* [x] `/proc/keys`
* [x] `/proc/key-users`
* [ ] `/proc/kmsg`
* [ ] `/proc/kpagecgroup`
* [ ] `/proc/kpagecgroup`
* [ ] `/proc/kpagecount`
* [ ] `/proc/kpageflags`
* [ ] `/proc/ksyms`
* [x] `/proc/loadavg`
* [x] `/proc/locks`
* [ ] `/proc/malloc`
* [x] `/proc/meminfo`
* [x] `/proc/modules`
* [ ] `/proc/mounts`
* [ ] `/proc/mtrr`
* [ ] `/proc/net`
  * [x] `/proc/net/arp`
  * [x] `/proc/net/dev`
  * [ ] `/proc/net/dev_mcast`
  * [ ] `/proc/net/igmp`
  * [ ] `/proc/net/ipv6_route`
  * [ ] `/proc/net/rarp`
  * [ ] `/proc/net/raw`
  * [x] `/proc/net/route`
  * [ ] `/proc/net/snmp`
  * [x] `/proc/net/tcp`
  * [x] `/proc/net/udp`
  * [x] `/proc/net/unix`
  * [ ] `/proc/net/netfilter/nfnetlink_queue`
* [ ] `/proc/partitions`
* [ ] `/proc/pci`
* [x] `/proc/pressure`
  * [x] `/proc/pressure/cpu`
  * [x] `/proc/pressure/io`
  * [x] `/proc/pressure/memory`
* [ ] `/proc/profile`
* [ ] `/proc/scsi`
* [ ] `/proc/scsi/scsi`
* [ ] `/proc/scsi/[drivername]`
* [ ] `/proc/self`
* [ ] `/proc/slabinfo`
* [x] `/proc/stat`
* [ ] `/proc/swaps`
* [ ] `/proc/sys`
  * [ ] `/proc/sys/abi`
  * [ ] `/proc/sys/debug`
  * [ ] `/proc/sys/dev`
  * [ ] `/proc/sys/fs`
	* [x] `/proc/sys/fs/binfmt_misc`
	* [x] `/proc/sys/fs/dentry-state`
	* [ ] `/proc/sys/fs/dir-notify-enable`
	* [ ] `/proc/sys/fs/dquot-max`
	* [ ] `/proc/sys/fs/dquot-nr`
	* [x] `/proc/sys/fs/epoll`
	* [x] `/proc/sys/fs/file-max`
	* [x] `/proc/sys/fs/file-nr`
	* [ ] `/proc/sys/fs/inode-max`
	* [ ] `/proc/sys/fs/inode-nr`
	* [ ] `/proc/sys/fs/inode-state`
	* [ ] `/proc/sys/fs/inotify`
	* [ ] `/proc/sys/fs/lease-break-time`
	* [ ] `/proc/sys/fs/leases-enable`
	* [ ] `/proc/sys/fs/mount-max`
	* [ ] `/proc/sys/fs/mqueue`
	* [ ] `/proc/sys/fs/nr_open`
	* [ ] `/proc/sys/fs/overflowgid`
	* [ ] `/proc/sys/fs/overflowuid`
	* [ ] `/proc/sys/fs/pipe-max-size`
	* [ ] `/proc/sys/fs/pipe-user-pages-hard`
	* [ ] `/proc/sys/fs/pipe-user-pages-soft`
	* [ ] `/proc/sys/fs/protected_hardlinks`
	* [ ] `/proc/sys/fs/protected_symlinks`
	* [ ] `/proc/sys/fs/suid_dumpable`
	* [ ] `/proc/sys/fs/super-max`
	* [ ] `/proc/sys/fs/super-nr`
  * [ ] `/proc/sys/kernel`
	* [ ] `/proc/sys/kernel/acct`
	* [ ] `/proc/sys/kernel/auto_msgmni`
	* [ ] `/proc/sys/kernel/cap_last_cap`
	* [ ] `/proc/sys/kernel/cap-bound`
	* [ ] `/proc/sys/kernel/core_pattern`
	* [ ] `/proc/sys/kernel/core_pipe_limit`
	* [ ] `/proc/sys/kernel/core_uses_pid`
	* [ ] `/proc/sys/kernel/ctrl-alt-del`
	* [ ] `/proc/sys/kernel/dmesg_restrict`
	* [ ] `/proc/sys/kernel/domainname`
	* [ ] `/proc/sys/kernel/hostname`
	* [ ] `/proc/sys/kernel/hotplug`
	* [ ] `/proc/sys/kernel/htab-reclaim`
	* [x] `/proc/sys/kernel/keys/\*`
	* [ ] `/proc/sys/kernel/kptr_restrict`
	* [ ] `/proc/sys/kernel/l2cr`
	* [ ] `/proc/sys/kernel/modprobe`
	* [ ] `/proc/sys/kernel/modules_disabled`
	* [ ] `/proc/sys/kernel/msgmax`
	* [ ] `/proc/sys/kernel/msgmni`
	* [ ] `/proc/sys/kernel/msgmnb`
	* [ ] `/proc/sys/kernel/ngroups_max`
	* [ ] `/proc/sys/kernel/ns_last_pid`
	* [x] `/proc/sys/kernel/ostype`
	* [x] `/proc/sys/kernel/osrelease`
	* [ ] `/proc/sys/kernel/overflowgid`
	* [ ] `/proc/sys/kernel/overflowuid`
	* [ ] `/proc/sys/kernel/panic`
	* [ ] `/proc/sys/kernel/panic_on_oops`
	* [x] `/proc/sys/kernel/pid_max`
	* [ ] `/proc/sys/kernel/powersave-nap`
	* [ ] `/proc/sys/kernel/printk`
	* [ ] `/proc/sys/kernel/pty`
	* [ ] `/proc/sys/kernel/pty/max`
	* [ ] `/proc/sys/kernel/pty/nr`
	* [x] `/proc/sys/kernel/random`
		* [x] `/proc/sys/kernel/random/entropy_avail`
		* [x] `/proc/sys/kernel/random/poolsize`
		* [x] `/proc/sys/kernel/random/read_wakeup_threshold`
		* [x] `/proc/sys/kernel/random/write_wakeup_threshold`
		* [x] `/proc/sys/kernel/random/uuid`
		* [x] `/proc/sys/kernel/random/boot_id`
	* [ ] `/proc/sys/kernel/randomize_va_space`
	* [ ] `/proc/sys/kernel/real-root-dev`
	* [ ] `/proc/sys/kernel/reboot-cmd`
	* [ ] `/proc/sys/kernel/rtsig-max`
	* [ ] `/proc/sys/kernel/rtsig-nr`
	* [ ] `/proc/sys/kernel/sched_child_runs_first`
	* [ ] `/proc/sys/kernel/sched_rr_timeslice_ms`
	* [ ] `/proc/sys/kernel/sched_rt_period_us`
	* [ ] `/proc/sys/kernel/sched_rt_runtime_us`
	* [ ] `/proc/sys/kernel/seccomp`
	* [x] `/proc/sys/kernel/sem`
	* [ ] `/proc/sys/kernel/sg-big-buff`
	* [ ] `/proc/sys/kernel/shm_rmid_forced`
	* [x] `/proc/sys/kernel/shmall`
	* [x] `/proc/sys/kernel/shmmax`
	* [x] `/proc/sys/kernel/shmmni`
	* [ ] `/proc/sys/kernel/sysctl_writes_strict`
	* [x] `/proc/sys/kernel/sysrq`
	* [x] `/proc/sys/kernel/version`
	* [x] `/proc/sys/kernel/threads-max`
	* [ ] `/proc/sys/kernel/yama/ptrace_scope`
	* [ ] `/proc/sys/kernel/zero-paged`
  * [ ] `/proc/sys/net`
	* [ ] `/proc/sys/net/core/bpf_jit_enable`
	* [ ] `/proc/sys/net/core/somaxconn`
  * [ ] `/proc/sys/proc`
  * [ ] `/proc/sys/sunrpc`
  * [ ] `/proc/sys/user`
  * [ ] `/proc/sys/vm`
	* [x] `/proc/sys/vm/admin_reserve_kbytes`
	* [ ] `/proc/sys/vm/compact_memory`
	* [x] `/proc/sys/vm/drop_caches`
	* [ ] `/proc/sys/vm/legacy_va_layout`
	* [ ] `/proc/sys/vm/memory_failure_early_kill`
	* [ ] `/proc/sys/vm/memory_failure_recovery`
	* [ ] `/proc/sys/vm/oom_dump_tasks`
	* [ ] `/proc/sys/vm/oom_kill_allocating_task`
	* [ ] `/proc/sys/vm/overcommit_kbytes`
	* [x] `/proc/sys/vm/overcommit_memory`
	* [ ] `/proc/sys/vm/overcommit_ratio`
	* [ ] `/proc/sys/vm/panic_on_oom`
	* [ ] `/proc/sys/vm/swappiness`
	* [ ] `/proc/sys/vm/user_reserve_kbytes`
* [ ] `/proc/sysrq-trigger`
* [ ] `/proc/sysvipc`
* [ ] `/proc/thread-self`
* [ ] `/proc/timer_list`
* [ ] `/proc/timer_stats`
* [ ] `/proc/tty`
* [x] `/proc/uptime`
* [ ] `/proc/version`
* [x] `/proc/vmstat`
* [ ] `/proc/zoneinfo`
