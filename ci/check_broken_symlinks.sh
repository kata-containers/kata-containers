#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# check_broken_symlinks.sh - Static check that every symbolic link tracked in
# the repository resolves to an existing file inside the repository.
#
# Downstream builders that package a checkout of this repository bail out on
# dangling links, and links pointing outside the repository resolve by accident
# (or not at all) depending on where the checkout lives.
#
# Usage: ./ci/check_broken_symlinks.sh

set -o errexit
set -o nounset
set -o pipefail

[[ -n "${DEBUG:-}" ]] && set -o xtrace

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd -P)"

cd "${repo_root}"

# The file mode git records for a symbolic link, as printed in the first
# field of "git ls-files -s" (regular files are 100644 or 100755).
SYMLINK_MODE="120000"

# Only tracked files are considered: build artefacts left over in a working
# tree carry plenty of legitimately dangling links of their own.
links=()
while IFS= read -r -d '' entry; do
	# Each entry is "<mode> <object> <stage>\t<path>".
	[[ "${entry}" == "${SYMLINK_MODE} "* ]] || continue
	links+=("${entry#*$'\t'}")
done < <(git ls-files -sz)

echo "Checking ${#links[@]} tracked symbolic links..."

dangling=()
escaping=()

for link in "${links[@]}"; do
	target="$(readlink "${link}")"
	resolved="$(realpath -m "$(dirname "${link}")/${target}")"

	if [[ "${resolved}" != "${repo_root}" && "${resolved}" != "${repo_root}/"* ]]; then
		escaping+=("${link} -> ${target}")
	elif [[ ! -e "${link}" ]]; then
		dangling+=("${link} -> ${target}")
	fi
done

failed=0

if (( ${#dangling[@]} > 0 )); then
	echo -e "\nFAIL: the following symbolic links do not resolve to an existing file:"
	for entry in "${dangling[@]}"; do
		echo "  - ${entry}"
	done
	failed=1
fi

if (( ${#escaping[@]} > 0 )); then
	echo -e "\nFAIL: the following symbolic links point outside of the repository:"
	for entry in "${escaping[@]}"; do
		echo "  - ${entry}"
	done
	failed=1
fi

if (( failed == 0 )); then
	echo "OK: all ${#links[@]} tracked symbolic links resolve inside the repository."
fi

exit "${failed}"
