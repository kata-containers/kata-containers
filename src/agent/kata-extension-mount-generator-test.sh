#!/bin/bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Run with: bash src/agent/kata-extension-mount-generator-test.sh
# All cmdline reads and generated output use temporary fixtures, never /proc.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
generator="${script_dir}/kata-extension-mount-generator.sh"
unit="/usr/lib/systemd/system/kata-extension-mount@.service"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/kata-extension-generator-test.XXXXXX")"
trap 'rm -rf -- "${test_dir}"' EXIT
tests=0

fail() {
	echo "FAIL: ${case_name}: $*" >&2
	exit 1
}

new_case() {
	case_name="$1"
	case_dir="${test_dir}/${case_name}"
	normal_dir="${case_dir}/normal"
	early_dir="${case_dir}/early"
	late_dir="${case_dir}/late"
	cmdline_file="${case_dir}/cmdline"
	wants_dir="${normal_dir}/kata-containers.target.wants"
	dropin="${normal_dir}/kata-agent.service.d/50-apparmor-extension.conf"
	mkdir -p "${normal_dir}" "${early_dir}" "${late_dir}"
	printf '%s\n' "$2" > "${cmdline_file}"
}

run_generator() {
	# Set the override on every invocation, including missing/empty fixtures.
	(cd -- "${case_dir}" && KATA_EXTENSION_CMDLINE_FILE="${cmdline_file}" \
		bash "${generator}" "${normal_dir}" "${early_dir}" "${late_dir}")
}

assert_wants() {
	local name
	local count
	for name in "$@"; do
		local link="${wants_dir}/kata-extension-mount@${name}.service"
		[[ -L "${link}" ]] || fail "missing Wants for ${name}"
		[[ "$(readlink -- "${link}")" == "${unit}" ]] || fail "wrong template for ${name}"
	done
	count="$(find "${normal_dir}" -type l | wc -l)"
	[[ "${count}" -eq "$#" ]] || fail "unexpected dependency symlinks"
}

assert_agent_dependency() {
	local expected=$'[Unit]\nRequires=kata-extension-mount@apparmor.service\nAfter=kata-extension-mount@apparmor.service'
	[[ -f "${dropin}" ]] || fail "missing AppArmor agent drop-in"
	[[ "$(< "${dropin}")" == "${expected}" ]] || \
		fail "agent must require and follow only the AppArmor mount"
}

assert_no_agent_dependency() {
	[[ ! -e "${normal_dir}/kata-agent.service.d" ]] || fail "unexpected agent drop-in"
	[[ ! -e "${normal_dir}/kata-agent.service.requires" ]] || fail "unexpected hard dependency"
}

assert_empty_output() {
	[[ -z "$(find "${normal_dir}" -mindepth 1 -print -quit)" ]] || fail "unexpected generated units"
}

pass() {
	[[ -z "$(find "${early_dir}" "${late_dir}" -mindepth 1 -print -quit)" ]] || \
		fail "output must only use the normal generator directory"
	tests=$((tests + 1))
	echo "ok ${tests} - ${case_name}"
}

# Preserve the mount template's ordering and existing failure shutdown path.
case_name="mount-template-contract"
grep -Fxq 'Before=kata-agent.service' "${script_dir}/kata-extension-mount@.service" || \
	fail "mount template lost agent ordering"
grep -Fxq 'OnFailure=poweroff.target' "${script_dir}/kata-extension-mount@.service" || \
	fail "mount template lost failure shutdown"

new_case no-extension 'console=ttyS0 root=/dev/vda1'
run_generator
assert_empty_output
assert_no_agent_dependency
pass

new_case apparmor 'kata.extension.apparmor.verity_params=root_hash=abc,salt=def'
run_generator
assert_wants apparmor
assert_agent_dependency
pass

new_case bare-verity 'kata.extension.apparmor.verity_params'
run_generator
assert_wants apparmor
assert_agent_dependency
pass

new_case empty-verity 'kata.extension.apparmor.verity_params='
run_generator
assert_wants apparmor
assert_agent_dependency
pass

new_case other-extensions 'kata.extension.nvidia_gpu.verity_params=root_hash=123 kata.extension.coco-guest.verity_params='
run_generator
assert_wants nvidia_gpu coco-guest
assert_no_agent_dependency
pass

new_case mixed-extensions 'kata.extension.nvidia_gpu.verity_params=root_hash=123 kata.extension.apparmor.verity_params=root_hash=abc kata.extension.coco-guest.verity_params'
run_generator
assert_wants nvidia_gpu apparmor coco-guest
assert_agent_dependency
pass

new_case invalid-names 'kata.extension..verity_params=x kata.extension.../apparmor.verity_params=x kata.extension.apparmor/foo.verity_params=x kata.extension.apparmor@evil.verity_params=x kata.extension.apparmor%evil.verity_params=x kata.extension.apparmor;evil.verity_params=x kata.extension.apparmor=evil.verity_params=x kata.extension.apparmor\evil.verity_params=x kata.extension.apparmor.extra.verity_params=x'
run_generator
assert_empty_output
assert_no_agent_dependency
pass

printf -v long_name '%0226d' 0
new_case longest-valid-name "kata.extension.${long_name}.verity_params="
run_generator
assert_wants "${long_name}"
assert_no_agent_dependency
pass

long_name="${long_name}0"
new_case overlong-name "kata.extension.${long_name}.verity_params=x kata.extension.apparmor.verity_params="
run_generator
assert_wants apparmor
assert_agent_dependency
pass

new_case invalid-with-valid 'kata.extension.../apparmor.verity_params=x kata.extension.apparmor@evil.verity_params=x kata.extension.coco-guest.verity_params kata.extension.apparmor.verity_params='
run_generator
assert_wants coco-guest apparmor
assert_agent_dependency
pass

new_case apparmor-lookalikes 'kata.extension.AppArmor.verity_params=x kata.extension.apparmor-extra.verity_params=x kata.extension.apparmor_extra.verity_params=x apparmor=1 kata.extension.apparmor.verity_params_extra=x'
run_generator
assert_wants AppArmor apparmor-extra apparmor_extra
assert_no_agent_dependency
pass

new_case glob-name 'kata.extension.*.verity_params=*'
# An unquoted cmdline loop must not expand this wildcard into a valid name.
touch "${case_dir}/kata.extension.apparmor.verity_params=forged"
run_generator
assert_empty_output
assert_no_agent_dependency
pass

new_case duplicate-apparmor 'kata.extension.apparmor.verity_params=root_hash=abc kata.extension.apparmor.verity_params= kata.extension.apparmor.verity_params'
run_generator
run_generator
assert_wants apparmor
assert_agent_dependency
[[ "$(find "${normal_dir}" -type f | wc -l)" -eq 1 ]] || fail "duplicate drop-ins"
pass

new_case empty-cmdline ''
truncate -s 0 "${cmdline_file}"
run_generator
assert_empty_output
pass

new_case missing-cmdline ''
cmdline_file="${case_dir}/missing-cmdline"
run_generator 2> "${case_dir}/stderr"
assert_empty_output
pass

new_case no-final-newline ''
printf '%s' 'kata.extension.apparmor.verity_params=' > "${cmdline_file}"
run_generator
assert_wants apparmor
assert_agent_dependency
pass

echo "PASS: ${tests} generator cases (fixture cmdline only)"
