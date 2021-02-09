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
        if line=="":
            continue
        data = json.loads(line)
        if data["type"]== "match":
            matches.append(data)

    return matches

def get_resources(matches):
    resources = {}
    for match in matches:
        line = match["data"]["lines"]["text"]
        m = re.match(".+RLIMIT_([^_]+?):",line)
        if m is None:
            continue
        c_enum_name = m.group(1)
        resource_id = c_enum_name.split("_")[0]
        file_path = match["data"]["path"]["text"]
        rel_file_path = re.match(".+src/(.+)",file_path).group(1)
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
    'unix/haiku/mod.rs':                        'target_os = "haiku"',
    'unix/linux_like/android/mod.rs':           'target_os = "android"',
    'unix/linux_like/emscripten/mod.rs':        'target_os = "emscripten"',
    'unix/linux_like/linux/gnu':                'all(target_os = "linux", target_env = "gnu")',
    'unix/linux_like/linux/musl/b64':           'all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64"))',
    'unix/linux_like/linux/musl/b32':           'all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm"))',
    'unix/linux_like/linux/musl/mod.rs':        'all(target_os = "linux", target_env = "musl")',
    'unix/solarish/mod.rs':                     'target_os = "solarish"',
    'unix/uclibc/mips/mod.rs':                  'all(target_env = "uclibc",any(target_arch = "mips", target_arch = "mips64"))',
    'unix/uclibc/mod.rs':                       'target_env = "uclibc"',
    'unix/bsd/freebsdlike/dragonfly/mod.rs':    'target_os = "dragonfly"',
    'unix/bsd/freebsdlike/freebsd/mod.rs':      'target_os = "freebsd"',
}

def get_selectors(resources):
    selectors = {}
    for resource_id in resources:
        resources_selectors = {}
        for path in resources[resource_id]:
            for selector in SELECTORS:
                if path.startswith(selector):
                    if selector in resources_selectors:
                        break
                    resources_selectors[selector] =SELECTORS[selector]
                    break
            else:
                raise Exception(f'can not find a selector for {path}, id = {resource_id}')
        selectors[resource_id] = resources_selectors
    return selectors

def codegen(selectors):
    code = []
    for resource_id in selectors:
        selector = "#[cfg(any(\n    " + ',\n    '.join(selectors[resource_id].values()) + "\n))]"
        declaration = f'{resource_id}: RLIMIT_{resource_id},'
        code.append((resource_id,f'{selector}\n{declaration}\n'))
    code.sort(key=lambda x:x[0])
    for decl in code:
        print(decl[1])

matches = get_matches()
resources = get_resources(matches)
selectors = get_selectors(resources)
codegen(selectors)
