#!/usr/bin/python3
import os
import json
import re
from typing import Any, Dict, List

from . import libc_source

docs = {}

docs[
    "AS"
] = """
    /// The maximum size (in bytes)
    /// of the process's virtual memory (address space).
"""

docs[
    "CORE"
] = """
    /// The maximum size (in bytes)
    /// of a core file that the process may dump.
"""

docs[
    "CPU"
] = """
    /// A limit (in seconds)
    /// on the amount of CPU time that the process can consume.
"""

docs[
    "DATA"
] = """
    /// The maximum size (in bytes)
    /// of the process's data segment
    /// (initialized data, uninitialized data, and heap).
"""

docs[
    "FSIZE"
] = """
    /// The maximum size (in bytes)
    /// of files that the process may create.
"""

docs[
    "KQUEUES"
] = """
    /// The maximum number of kqueues this user id is allowed to create.
"""

docs[
    "LOCKS"
] = """
    /// (early Linux 2.4 only)
    ///
    /// A limit on the combined number
    /// of `flock(2)` locks and `fcntl(2)` leases
    /// that this process may establish.
"""

docs[
    "MEMLOCK"
] = """
    /// The maximum number (in bytes)
    /// of memory that may be locked into RAM.
"""

docs[
    "MSGQUEUE"
] = """
    /// A limit on the number
    /// of bytes that can be allocated for POSIX message queues
    /// for the real user ID of the calling process.
"""

docs[
    "NICE"
] = """
    /// This specifies a ceiling
    /// to which the process's nice value can be raised
    /// using `setpriority(2)` or `nice(2)`.
"""

docs[
    "NOFILE"
] = """
    /// This specifies a value
    /// one greater than the maximum file descriptor number
    /// that can be opened by this process.
"""

docs[
    "NOVMON"
] = """
    /// The number of open vnode monitors.
"""

docs[
    "NPROC"
] = """
    /// A limit on the number of extant process (or, more precisely on Linux, threads)
    /// for the real user ID of the calling process.
"""

docs[
    "NPTS"
] = """
    /// The maximum number of pseudo-terminals this user id is allowed to create.
"""

docs[
    "NTHR"
] = """
    /// The maximum number of simultaneous threads (Lightweight
    /// Processes) for this user id.  Kernel threads and the
    /// first thread of each process are not counted against this
    /// limit.
"""

docs[
    "POSIXLOCKS"
] = """
    /// The maximum number of POSIX-type advisory-mode locks available to this user.
"""

docs[
    "RSS"
] = """
    /// A limit (in bytes)
    /// on the process's resident set
    /// (the number of virtual pages resident in RAM).
"""

docs[
    "RTPRIO"
] = """
    /// This specifies a ceiling on the real-time priority
    /// that may be set for this process
    /// using `sched_setscheduler(2)` and `sched_setparam(2)`.
"""

docs[
    "RTTIME"
] = """
    /// A limit (in microseconds) on the amount of CPU time
    /// that a process scheduled under a real-time scheduling policy
    /// may consume without making a blocking system call.
"""

docs[
    "SBSIZE"
] = """
    /// The maximum size (in bytes) of socket buffer usage for
    /// this user. This limits the amount of network memory, and
    /// hence the amount of mbufs, that this user may hold at any
    /// time.
"""

docs[
    "SIGPENDING"
] = """
    /// A limit on the number
    /// of signals that may be queued
    /// for the real user ID of the calling process.
"""

docs[
    "STACK"
] = """
    /// The maximum size (in bytes)
    /// of the process stack.
"""

docs[
    "SWAP"
] = """
    /// The maximum size (in bytes) of the swap space that may be
    /// reserved or used by all of this user id's processes.
"""

docs[
    "UMTXP"
] = """
    /// The number of shared locks a given user may create simultaneously.
"""

docs[
    "VMEM"
] = """
    /// An alias for RLIMIT_AS. The maximum size of a process's mapped address space in bytes.
"""


if __name__ == "__main__":
    resources = libc_source.search_ident("RLIMIT_.+?:", ".+[^_]RLIMIT_(.+?):")
    del resources["NLIMITS"]
    selectors = libc_source.calc_selectors(resources)

    print(f"// generated from rust-lang/libc {libc_source.COMMIT_HASH}")
    print("declare_resource! {")
    for tag, resource_id in enumerate(sorted(selectors.keys()), 1):
        print(docs[resource_id], end="")
        print(libc_source.calc_cfg(sorted(selectors[resource_id].values()), indent=4))
        print(f"    {resource_id} = {tag} => RLIMIT_{resource_id},\n")
    print("}")
