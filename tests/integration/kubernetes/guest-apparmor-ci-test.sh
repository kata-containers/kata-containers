#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

# Local assertions only: temporary render fixtures and rejected CI entrypoints.
# Never set runner credentials, mount images, load policy, or access a cluster.
set -euo pipefail
test_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
python="${PYTHON:-python3}"
"${python}" "${test_dir}/guest-apparmor-ci-config.py" self-test

scratch="$(mktemp -d)"
trap 'rm -rf -- "${scratch:?}"' EXIT
for command in install-test-extension configure configure-baseline cleanup verify-images; do
	if GITHUB_ACTIONS=false RUNNER_ENVIRONMENT=local KATA_GUEST_APPARMOR_TEST=no KATA_GUEST_APPARMOR_BASELINE=no \
		bash "${test_dir}/guest-apparmor-ci.sh" "${command}" > "${scratch}/output" 2>&1; then
		echo "ERROR: local runner unexpectedly accepted ${command}" >&2
		exit 1
	fi
	grep -Fq 'host changes require root, a disposable GitHub-hosted runner' "${scratch}/output"
	printf 'ok - local runner rejects %s before any host changes\n' "${command}"
done

# Exercise the install preflight in a temporary layout. tar and sha256sum are
# in-process mocks: no archive is ever extracted and /opt is never written.
for scenario in new existing dangling-link; do
	set +e
	(
		set -e
		# shellcheck source=/dev/null
		source <(awk '/^install_test_extension\(\) / { emit=1 } /^verify_image\(\) / { emit=0 } emit' "${test_dir}/guest-apparmor-ci.sh")
		die() { echo "ERROR: $*" >&2; exit 1; }
		require_disposable_runner() { :; }
		tar() { printf '%s\n' "$@" > "${scratch}/${scenario}.tar-args"; }
		sha256sum() { :; }
		install_root="${scratch}/${scenario}"
		images="${install_root}/share/kata-containers"
		mkdir -p "${images}"
		archive="${install_root}/kata-static-rootfs-image-apparmor-test-extension.tar.zst"
		printf fixture > "${archive}"
		case "${scenario}" in
			existing) printf preserved > "${images}/kata-containers-apparmor-test-extension.img" ;;
			dangling-link) ln -s nonexistent "${images}/root_hash_apparmor-test-extension.txt" ;;
		esac
		export KATA_GUEST_APPARMOR_TEST=yes
		install_test_extension "${archive}"
	) > "${scratch}/output" 2>&1
	result=$?
	set -e
	if [[ "${scenario}" == new ]]; then
		[[ "${result}" -eq 0 ]]
		grep -Fxq -- --keep-old-files "${scratch}/${scenario}.tar-args"
		[[ "$(grep -c '^\./opt/kata/' "${scratch}/${scenario}.tar-args")" -eq 2 ]]
		grep -Fxq './opt/kata/share/kata-containers/kata-containers-apparmor-test-extension.img' "${scratch}/${scenario}.tar-args"
		grep -Fxq './opt/kata/share/kata-containers/root_hash_apparmor-test-extension.txt' "${scratch}/${scenario}.tar-args"
	else
		[[ "${result}" -ne 0 && ! -e "${scratch}/${scenario}.tar-args" ]]
		grep -Fq 'refusing to overwrite existing test artifact' "${scratch}/output"
	fi
	printf 'ok - test artifact installation preflight: %s\n' "${scenario}"
done
[[ "$(cat "${scratch}/existing/share/kata-containers/kata-containers-apparmor-test-extension.img")" == preserved ]]
[[ -L "${scratch}/dangling-link/share/kata-containers/root_hash_apparmor-test-extension.txt" ]]
