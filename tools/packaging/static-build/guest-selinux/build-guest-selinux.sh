#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Runs inside the guest-selinux builder container: installs the Kata guest module
# into the container's own targeted policy store and lays out the selinux
# extension tree under ${DESTDIR}.

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

readonly policy_type="targeted"
# NVRC writes the policy straight to /sys/fs/selinux/load, without libselinux's
# downgrade to the running kernel's policyvers, so the version is pinned to one
# the guest kernel (v6.18: POLICYDB_VERSION_MAX = 35) accepts.
readonly policy_version="${GUEST_SELINUX_POLICY_VERSION:-35}"
readonly module_priority=400

readonly store_dir="/etc/selinux/${policy_type}"
readonly out_dir="${DESTDIR:?}/etc/selinux/kata"
readonly manifest_dir="${DESTDIR}/etc/kata-extensions"

sed -i '/^policy-version[[:space:]]*=/d' /etc/selinux/semanage.conf
echo "policy-version = ${policy_version}" >> /etc/selinux/semanage.conf

# -n: build the store without loading it; the builder has no SELinux of its own.
semodule -n -X "${module_priority}" -i "${script_dir}/kata-guest.cil"

policy="${store_dir}/policy/policy.${policy_version}"
[[ -f "${policy}" ]] || { echo "expected ${policy} to be built" >&2; exit 1; }

# Fail the build rather than ship a policy the guest cannot use.
for t in kata_agent_t kata_coco_t kata_nvidia_t container_t container_file_t; do
	seinfo "${policy}" -t "${t}" | grep -q "${t}" \
		|| { echo "type ${t} missing from ${policy}" >&2; exit 1; }
done
sesearch "${policy}" -T -s kernel_t -t kata_agent_exec_t -c process | grep -q kata_agent_t \
	|| { echo "kernel_t -> kata_agent_t transition missing" >&2; exit 1; }
# NVRC refuses to boot a policy it cannot lock.
seinfo "${policy}" -b secure_mode_policyload | grep -q secure_mode_policyload \
	|| { echo "secure_mode_policyload boolean missing from ${policy}" >&2; exit 1; }

install -D -m 0644 "${policy}" "${out_dir}/policy"
install -D -m 0644 "${store_dir}/contexts/files/file_contexts" "${out_dir}/file_contexts"
install -D -m 0644 "${script_dir}/extension_contexts" "${out_dir}/extension_contexts"
install -D -m 0644 "${script_dir}/components.toml" "${manifest_dir}/components.toml"

{
	echo "policy_version=${policy_version}"
	rpm -q selinux-policy-targeted container-selinux policycoreutils
} > "${out_dir}/build-info"
