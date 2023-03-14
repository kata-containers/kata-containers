#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# pyre-unsafe

import os
import subprocess
import sys


def rewrite_dep_file(src_path, dst_path):
    """
    Convert a makefile to a depfile suitable for use by Buck2. The files we
    rewrite look like P488268797.
    """
    here = os.getcwd().replace("\\", "/") + "/"

    with open(src_path) as f:
        body = f.read()

    parts = body.split(": ", 1)
    body = parts[1] if len(parts) == 2 else ""

    # Escaped newlines are not meaningful so remove them.
    body = body.replace("\\\n", "")

    # Now, recover targets. They are space separated, but we need to ignore
    # spaces that are escaped.
    pos = 0

    deps = []
    current_parts = []

    def push_slice(s):
        if s:
            current_parts.append(s)

    def flush_current_dep():
        if current_parts:
            deps.append("".join(current_parts))
            current_parts.clear()

    while True:
        next_pos = body.find(" ", pos)

        # If we find the same character we started at, this means we started on
        # a piece of whitespace. We know this cannot be escaped, because if we
        # started here that means we stopped at the previous character, which
        # means it must have been whitespace as well.
        if next_pos == pos:
            flush_current_dep()
            pos += 1
            continue

        # No more whitespace, so this means that whatever is left from our
        # current position to the end is the last dependency (assuming there is
        # anything).
        if next_pos < 0:
            push_slice(body[pos:-1])
            break

        # Check if this was escaped by looking at the previous character. If it
        # was, then insert the part before the escape, and then push a space.
        # If it wasn't, then we've reached the end of a dependency.
        if next_pos > 0 and body[next_pos - 1] == "\\":
            push_slice(body[pos : next_pos - 1])
            push_slice(" ")
        else:
            push_slice(body[pos:next_pos])
            flush_current_dep()

        pos = next_pos + 1

    flush_current_dep()

    # Now that we've parsed deps, we need to normalize them.

    normalized_deps = []

    for dep in deps:
        # The paths we get sometimes include "../" components, so get rid
        # of those because we want ForwardRelativePath here.
        dep = os.path.normpath(dep).replace("\\", "/")

        if os.path.isabs(dep):
            if dep.startswith(here):
                # The dep file included a path inside the build root, but
                # expressed an absolute path. In this case, rewrite it to
                # be a relative path.
                dep = dep[len(here) :]
            else:
                # The dep file included a path to something outside the
                # build root. That's bad (actions shouldn't depend on
                # anything outside the build root), but that dependency is
                # therefore not tracked by Buck2 (which can only see things
                # in the build root), so it cannot be represented as a
                # dependency and therefore we don't include it (event if we
                # could include it, this could never cause a miss).
                continue

        normalized_deps.append(dep)

    with open(dst_path, "w") as f:
        for dep in normalized_deps:
            f.write(dep)
            f.write("\n")


def main():
    """
    Expects the src dep file to be the first argument, dst dep file to be the
    second argument, and the command to follow.
    """
    ret = subprocess.call(sys.argv[3:])
    if ret == 0:
        rewrite_dep_file(sys.argv[1], sys.argv[2])
    sys.exit(ret)


if __name__ == "__main__":
    main()
