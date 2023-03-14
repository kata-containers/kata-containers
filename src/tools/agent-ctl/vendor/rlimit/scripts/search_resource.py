import os
import json
import re
from pprint import pprint

LIBC_SRC_PATH = os.getenv("LIBC_SRC_PATH")


def get_matches():
    pipe = os.popen(f'rg --json "RLIMIT_.+?:" {LIBC_SRC_PATH}')
    lines = pipe.read().split('\n')
    matches = []

    for line in lines:
        if line == "":
            continue
        data = json.loads(line)
        if data["type"] == "match":
            matches.append(data)

    return matches


def get_resources(matches):
    resources = {}
    for match in matches:
        line = match["data"]["lines"]["text"]
        m = re.match(".+RLIMIT_([^_]+?):", line)
        if m is None:
            continue
        c_enum_name = m.group(1)
        resource_id = c_enum_name.split("_")[0]
        file_path = match["data"]["path"]["text"]
        rel_file_path = re.match(".+src/(.+)", file_path).group(1)
        data = rel_file_path
        if resource_id in resources:
            resources[resource_id].append(data)
        else:
            resources[resource_id] = [data]
    del resources["NLIMITS"]
    for v in resources.values():
        v.sort()
    return resources


SELECTORS = {
    'fuchsia/mod.rs':                           'target_os = "fuchsia"',
    'unix/bsd/apple/mod.rs':                    'any(target_os = "macos", target_os = "ios")',
    'unix/bsd/freebsdlike/mod.rs':              'any(target_os = "freebsd", target_os = "dragonfly")',
    'unix/bsd/netbsdlike/mod.rs':               'any(target_os = "openbsd", target_os = "netbsd")',
    'unix/bsd/netbsdlike/netbsd/mod.rs':        'target_os = "netbsd"',
    # 'unix/bsd/netbsdlike/openbsd/mod.rs':       'target_os = "openbsd"',
    'unix/haiku/mod.rs':                        'target_os = "haiku"',
    'unix/linux_like/android/mod.rs':           'target_os = "android"',
    'unix/linux_like/emscripten/mod.rs':        'target_os = "emscripten"',
    'unix/linux_like/linux/gnu':                'all(target_os = "linux", target_env = "gnu")',
    'unix/linux_like/linux/musl/b64':           'all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64"))',
    'unix/linux_like/linux/musl/b32':           'all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm"))',
    'unix/linux_like/linux/musl/mod.rs':        'all(target_os = "linux", target_env = "musl")',
    'unix/linux_like/linux/uclibc/mod.rs':      'all(target_os = "linux", target_env = "uclibc")',
    'unix/linux_like/linux/uclibc/mips/mod.rs': 'all(target_os = "linux", target_env = "uclibc", any(target_arch = "mips", target_arch = "mips64"))',
    'unix/solarish/mod.rs':                     'target_os = "solarish"',
    'unix/bsd/freebsdlike/dragonfly/mod.rs':    'target_os = "dragonfly"',
    'unix/bsd/freebsdlike/freebsd/mod.rs':      'target_os = "freebsd"',
}


def get_selectors(resources):
    hits = {k: 0 for k in SELECTORS.keys()}
    selectors = {}
    for resource_id in resources:
        resource_selectors = {}
        for path in resources[resource_id]:
            for selector in SELECTORS:
                if path.startswith(selector):
                    hits[selector] += 1
                    if selector in resource_selectors:
                        break
                    resource_selectors[selector] = SELECTORS[selector]
                    break
            else:
                raise Exception(
                    f'can not find a selector for {path}, id = {resource_id}')
        selectors[resource_id] = resource_selectors
    for k in hits:
        assert hits[k] > 0, f'selector {k} has not been hit'
    return selectors


docs = {}

docs['AS'] = '''
    /// The maximum size (in bytes)
    /// of the process's virtual memory (address space).
'''

docs['CORE'] = '''
    /// The maximum size (in bytes)
    /// of a core file that the process may dump.
'''

docs['CPU'] = '''
    /// A limit (in seconds)
    /// on the amount of CPU time that the process can consume.
'''

docs['DATA'] = '''
    /// The maximum size (in bytes)
    /// of the process's data segment
    /// (initialized data, uninitialized data, and heap).
'''

docs['FSIZE'] = '''
    /// The maximum size (in bytes)
    /// of files that the process may create.
'''

docs['KQUEUES'] = '''
    /// The maximum number of kqueues this user id is allowed to create.
'''

docs['LOCKS'] = '''
    /// (early Linux 2.4 only)
    ///
    /// A limit on the combined number
    /// of `flock(2)` locks and `fcntl(2)` leases
    /// that this process may establish.
'''

docs['MEMLOCK'] = '''
    /// The maximum number (in bytes)
    /// of memory that may be locked into RAM.
'''

docs['MSGQUEUE'] = '''
    /// A limit on the number
    /// of bytes that can be allocated for POSIX message queues
    /// for the real user ID of the calling process.
'''

docs['NICE'] = '''
    /// This specifies a ceiling
    /// to which the process's nice value can be raised
    /// using `setpriority(2)` or `nice(2)`.
'''

docs['NOFILE'] = '''
    /// This specifies a value
    /// one greater than the maximum file descriptor number
    /// that can be opened by this process.
'''

docs['NOVMON'] = '''
    /// The number of open vnode monitors.
'''

docs['NPROC'] = '''
    /// A limit on the number of extant process (or, more precisely on Linux, threads)
    /// for the real user ID of the calling process.
'''

docs['NPTS'] = '''
    /// The maximum number of pseudo-terminals this user id is allowed to create.
'''

docs['NTHR'] = '''
    /// The maximum number of simultaneous threads (Lightweight
    /// Processes) for this user id.  Kernel threads and the
    /// first thread of each process are not counted against this
    /// limit.
'''

docs['POSIXLOCKS'] = '''
    /// The maximum number of POSIX-type advisory-mode locks available to this user.
'''

docs['RSS'] = '''
    /// A limit (in bytes)
    /// on the process's resident set
    /// (the number of virtual pages resident in RAM).
'''

docs['RTPRIO'] = '''
    /// This specifies a ceiling on the real-time priority
    /// that may be set for this process
    /// using `sched_setscheduler(2)` and `sched_setparam(2)`.
'''

docs['RTTIME'] = '''
    /// A limit (in microseconds) on the amount of CPU time
    /// that a process scheduled under a real-time scheduling policy
    /// may consume without making a blocking system call.
'''

docs['SBSIZE'] = '''
    /// The maximum size (in bytes) of socket buffer usage for
    /// this user. This limits the amount of network memory, and
    /// hence the amount of mbufs, that this user may hold at any
    /// time.
'''

docs['SIGPENDING'] = '''
    /// A limit on the number
    /// of signals that may be queued
    /// for the real user ID of the calling process.
'''

docs['STACK'] = '''
    /// The maximum size (in bytes)
    /// of the process stack.
'''

docs['SWAP'] = '''
    /// The maximum size (in bytes) of the swap space that may be
    /// reserved or used by all of this user id's processes.
'''

docs['UMTXP'] = '''
    /// The number of shared locks a given user may create simultaneously.
'''

docs['VMEM'] = '''
    /// An alias for RLIMIT_AS. The maximum size of a process's mapped address space in bytes.
'''


def codegen(selectors):
    resources = sorted(selectors.keys())
    code = []
    for tag, resource_id in enumerate(resources, 1):
        selector = ''.join(
            '\n        ' + v + ',' for v in selectors[resource_id].values())
        selector = f"    #[cfg(any({selector}\n    ))]"
        doc = docs[resource_id]
        declaration = f'    {resource_id} = {tag} => RLIMIT_{resource_id},'
        code.append((resource_id, f'{selector}{doc}{declaration}\n'))
    for decl in code:
        print(decl[1])


matches = get_matches()
resources = get_resources(matches)
selectors = get_selectors(resources)
codegen(selectors)
