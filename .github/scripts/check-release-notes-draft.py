# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

import subprocess
import sys


version, cspell = sys.argv[1:]
draft = sys.stdin.buffer.read()
notes = draft.decode("utf-8")
lines = notes.split("\n")
errors = []

if lines[0] != f"# Kata Containers {version}":
    errors.append(f"Start with '# Kata Containers {version}'.")
if "## Upgrading" not in lines:
    errors.append("Include the exact heading '## Upgrading'.")
if b"\r" in draft or not draft.endswith(b"\n"):
    errors.append("Use LF line endings and end the file with a newline.")
for number, line in enumerate(lines, start=1):
    if line.endswith((" ", "\t")):
        errors.append(f"Line {number} has trailing whitespace.")

for error in errors:
    print(error, file=sys.stderr)

spelling = subprocess.run(
    [
        cspell,
        "--config", ".cspell.yaml",
        "--no-progress", "--no-summary",
        f"stdin://docs/releases/{version}.md",
    ],
    input=draft,
    check=False,
)
sys.exit(spelling.returncode or bool(errors))
