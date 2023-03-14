import os
import re
import json
from contextlib import contextmanager
from typing import Dict, List

PATH = os.getenv("LIBC_SRC_PATH", "libc")


@contextmanager
def chdir(path):
    cwd = os.getcwd()
    os.chdir(path)
    yield
    os.chdir(cwd)


with chdir(PATH):
    COMMIT_HASH = os.popen("git rev-parse HEAD").read().strip()


SELECTORS = {
    "fuchsia/mod.rs": 'target_os = "fuchsia"',
    "unix/bsd/apple/mod.rs": 'any(target_os = "macos", target_os = "ios")',
    "unix/bsd/freebsdlike/mod.rs": 'any(target_os = "freebsd", target_os = "dragonfly")',
    "unix/bsd/netbsdlike/mod.rs": 'any(target_os = "openbsd", target_os = "netbsd")',
    "unix/bsd/netbsdlike/netbsd/mod.rs": 'target_os = "netbsd"',
    # 'unix/bsd/netbsdlike/openbsd/mod.rs':       'target_os = "openbsd"',
    "unix/haiku/mod.rs": 'target_os = "haiku"',
    "unix/linux_like/android/mod.rs": 'target_os = "android"',
    "unix/linux_like/emscripten/mod.rs": 'target_os = "emscripten"',
    "unix/linux_like/linux/mod.rs": 'target_os = "linux"',
    "unix/linux_like/linux/gnu": 'all(target_os = "linux", target_env = "gnu")',
    "unix/linux_like/linux/musl/b64": 'all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64"))',
    "unix/linux_like/linux/musl/b32": 'all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm"))',
    "unix/linux_like/linux/musl/mod.rs": 'all(target_os = "linux", target_env = "musl")',
    "unix/linux_like/linux/uclibc/mod.rs": 'all(target_os = "linux", target_env = "uclibc")',
    "unix/linux_like/linux/uclibc/mips/mod.rs": 'all(target_os = "linux", target_env = "uclibc", any(target_arch = "mips", target_arch = "mips64"))',
    "unix/linux_like/linux/uclibc/mips/mips32/mod.rs": 'all(target_os = "linux", target_env = "uclibc", target_arch = "mips")',
    "unix/linux_like/linux/uclibc/mips/mips64/mod.rs": 'all(target_os = "linux", target_env = "uclibc", target_arch = "mips64")',
    "unix/linux_like/linux/uclibc/arm/mod.rs": 'all(target_os = "linux", target_env = "uclibc", target_arch = "arm")',
    "unix/linux_like/linux/uclibc/x86_64/mod.rs": 'all(target_os = "linux", target_env = "uclibc", target_arch = "x86_64")',
    "unix/solarish/mod.rs": 'target_os = "solarish"',
    "unix/bsd/freebsdlike/dragonfly/mod.rs": 'target_os = "dragonfly"',
    "unix/bsd/freebsdlike/freebsd/mod.rs": 'target_os = "freebsd"',
}


def search_ident(line_pattern: str, ident_pattern: str) -> Dict[str, List[str]]:
    pipe = os.popen(f"rg --json '{line_pattern}' {PATH}")
    lines = [l for l in pipe.read().split("\n") if l != ""]
    matches = []
    for line in lines:
        data = json.loads(line)
        if data["type"] == "match":
            matches.append(data)

    idents: Dict[str, List[str]] = {}
    for match in matches:
        line = match["data"]["lines"]["text"]
        m = re.match(ident_pattern, line)
        if m is None:
            continue
        ident = m.group(1)
        file_path = match["data"]["path"]["text"]
        rel_file_path = re.match(".+src/(.+)", file_path).group(1)  # type: ignore
        paths = idents.get(ident, [])
        paths.append(rel_file_path)
        idents[ident] = paths
    for v in idents.values():
        v.sort()
    return idents


def calc_selectors(idents: Dict[str, List[str]]) -> Dict[str, Dict[str, str]]:
    selectors: Dict[str, Dict[str, str]] = {}
    for ident in idents:
        ident_selectors: Dict[str, str] = {}
        for path in idents[ident]:
            for rel_path in SELECTORS:
                if path.startswith(rel_path):
                    if rel_path in ident_selectors:
                        break
                    ident_selectors[rel_path] = SELECTORS[rel_path]
                    break
            else:
                raise Exception(f"can not find a selector for {path}, id = {ident}")
        selectors[ident] = ident_selectors
    return selectors


def calc_cfg(selectors: List[str], *, indent: int, inverse: bool = False) -> str:
    indent_s = " " * indent
    lines = []
    if inverse:
        lines.append(f"{indent_s}#[cfg(not(any(\n")
    else:
        lines.append(f"{indent_s}#[cfg(any(\n")
    lines.extend(f"{indent_s}    {v},\n" for v in selectors)
    if inverse:
        lines.append(f"{indent_s})))]")
    else:
        lines.append(f"{indent_s}))]")
    return "".join(lines)
