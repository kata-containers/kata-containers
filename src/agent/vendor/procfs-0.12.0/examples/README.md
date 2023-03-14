# Examples

These examples can be run by running `cargo run --example example_name`

## dump.rs

Prints out details about the current process (the dumper itself), or a process specifed by PID

## interface_stats.rs

Runs continually and prints out how many bytes/packets are sent/received.  Press ctrl-c to exit the example:

```text
       Interface: bytes recv                         bytes sent
================  ====================               ====================
 br-883c4c992deb: 823307769                0.2 kbps  1537694158               0.5 kbps
 br-d73af6e6d094: 9137600399               0.9 kbps  2334717319               0.4 kbps
         docker0: 2938964881               0.6 kbps  19291691656             11.4 kbps
 docker_gwbridge: 1172300                  0.0 kbps  15649536                 0.0 kbps
        enp5s0f0: 44643307888420        5599.8 kbps  1509415976135           99.0 kbps
        enp5s0f1: 0                        0.0 kbps  0                        0.0 kbps
              lo: 161143108162             0.4 kbps  161143108162             0.4 kbps
     veth3154ff3: 3809619534               1.0 kbps  867529906                0.4 kbps
     veth487bc9b: 2650532684               0.8 kbps  2992458899               0.9 kbps
     veth8cb8ca8: 3234030733               0.7 kbps  16921098378             11.4 kbps
     vethbadbe14: 12007615348              3.8 kbps  15583195644              5.0 kbps
     vethc152f93: 978828                   0.0 kbps  3839134                  0.0 kbps
     vethe481f30: 1637142                  0.0 kbps  15805768                 0.0 kbps
     vethfac2e83: 19445827683              6.2 kbps  16194181515              5.1 kbps

```

## netstat.rs

Prints out all open and listening TCP/UDP sockets, along with the owning process.  The
output format is very similar to the standard `netstat` linux utility:

```text
Local address              Remote address             State           Inode    PID/Program name
0.0.0.0:53                 0.0.0.0:0                  Listen          30883        1409/pdns_server
0.0.0.0:51413              0.0.0.0:0                  Listen          24263        927/transmission-da
0.0.0.0:35445              0.0.0.0:0                  Listen          21777        942/rpc.mountd
0.0.0.0:22                 0.0.0.0:0                  Listen          27973        1149/sshd
0.0.0.0:25                 0.0.0.0:0                  Listen          28295        1612/master
```

## pressure.rs

Prints out CPU/IO/Memory pressure information

## ps.rs

Prints out all processes that share the same tty as the current terminal.  This is very similar to the standard
`ps` utility on linux when run with no arguments:

```text
  PID TTY          TIME CMD
 8369 pty/13       4.05 bash
23124 pty/13       0.23 basic-http-serv
24206 pty/13       0.11 ps
```

## self_memory.rs

Shows several ways to get the current memory usage of the current process

```text
PID: 21867
Memory page size: 4096
== Data from /proc/self/stat:
Total virtual memory used: 3436544 bytes
Total resident set: 220 pages (901120 bytes)

== Data from /proc/self/statm:
Total virtual memory used: 839 pages (3436544 bytes)
Total resident set: 220 pages (901120 byte)s
Total shared memory: 191 pages (782336 bytes)

== Data from /proc/self/status:
Total virtual memory used: 3436544 bytes
Total resident set: 901120 bytes
Total shared memory: 782336 bytes
```

## lsmod.rs

This lists all the loaded kernel modules, in a simple tree format.

## diskstat.rs

Lists IO information for local disks:

```text
sda1 mounted on /:
  total reads: 7325390 (13640070 ms)
  total writes: 124191552 (119109541 ms)
  total flushes: 0 (0 ms)
```

Note: only local disks will be shown (not NFS mounts,
and disks used for ZFS will not be shown either).

## lslocks.rs

Shows current file locks in a format that is similiar to the `lslocks` utility.

## mountinfo.rs

Lists all mountpoints, along with their type and options:

```text
sysfs on /sys type sysfs (noexec,relatime,nodev,rw,nosuid)
proc on /proc type proc (noexec,rw,nodev,relatime,nosuid)
udev on /dev type devtmpfs (rw,nosuid,relatime)
  mode = 755
  nr_inodes = 4109298
  size = 16437192k
devpts on /dev/pts type devpts (nosuid,rw,noexec,relatime)
  gid = 5
  ptmxmode = 000
  mode = 620
tmpfs on /run type tmpfs (rw,nosuid,noexec,relatime)
  size = 3291852k
  mode = 755
/dev/sda1 on / type ext4 (rw,relatime)
  errors = remount-ro
```

## process_hierarchy.rs

Lists all processes as a tree. Sub-processes will be hierarchically ordered beneath their parents.

```text
1       /usr/lib/systemd/systemd --system --deserialize 54
366         /usr/lib/systemd/systemd-journald
375         /usr/lib/systemd/systemd-udevd
383         /usr/bin/lvmetad -f
525         /usr/bin/dbus-daemon --system --address=systemd: --nofork --nopidfile --systemd-activation --syslog-only
529         /usr/bin/syncthing -no-browser -no-restart -logflags=0
608             /usr/bin/syncthing -no-browser -no-restart -logflags=0
530         /usr/lib/systemd/systemd-logind
...
```

