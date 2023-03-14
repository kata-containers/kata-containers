#!/usr/bin/env python3

import subprocess
import sys
from pathlib import Path


def powerset(input):
    if len(input) == 0:
        return [[]]

    pivot = input[0]

    subset = powerset(input[1:])
    with_pivot = subset.copy()
    for i, set in enumerate(with_pivot):
        with_pivot[i] = [pivot] + set

    return subset + with_pivot


def execute(args, **kwargs):
    cwd = ""
    if "cwd" in kwargs:
        cwd += str(kwargs["cwd"]) + "/ "
    print(cwd + "$ " + " ".join(args))
    status = subprocess.run(args, **kwargs)

    if status.returncode != 0:
        sys.exit(1)


def check(toolchain, features, **kwargs):
    for subset in powerset(features):
        feature_str = ",".join(subset)
        execute(
            ["cargo", "+" + toolchain, "check", "--features", feature_str],
            **kwargs
        )


features = [
    "alloc",
    "std",
    # "unstable",
    "compat_hash",
    "compat_macros",
]

check("stable", features)
check("nightly", features + ["unstable"])

for dir in Path("example-crates").iterdir():
    if not dir.joinpath("Cargo.toml").exists():
        continue
    execute(["cargo", "test", "--features", "std"], cwd=dir)

execute(["cargo", "test"], cwd="example-crates")
