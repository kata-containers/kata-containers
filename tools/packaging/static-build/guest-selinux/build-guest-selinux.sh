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

sed -i '/^\(policy-version\|handle-unknown\)[[:space:]]*=/d' /etc/selinux/semanage.conf
echo "policy-version = ${policy_version}" >> /etc/selinux/semanage.conf
# A permission the policy does not define is denied, not silently allowed.
echo "handle-unknown = deny" >> /etc/selinux/semanage.conf

# The guest kernel checks permissions that Fedora's base module does not
# declare yet. A module cannot extend a class, so add them to base. Only the
# allows in kata-guest.cil grant any of them.
readonly base_cil="/tmp/base.cil"
bzcat "/var/lib/selinux/${policy_type}/active/modules/100/base/cil" > "${base_cil}"
sed -i \
	-e 's/^(class system (\(.*\) ))$/(class system (\1 firmware_load kexec_image_load kexec_initramfs_load policy_load x509_certificate_load ))/' \
	-e 's/^(class io_uring (\(.*\) ))$/(class io_uring (\1 allowed ))/' \
	"${base_cil}"
if ! grep -q '^(class system (.* firmware_load ' "${base_cil}" \
	|| ! grep -q '^(class io_uring (.* allowed ))$' "${base_cil}"; then
	echo "could not extend the base system and io_uring classes" >&2
	exit 1
fi

# -n: build the store without loading it; the builder has no SELinux of its own.
semodule -n -X 100 -i "${base_cil}"
semodule -n -X "${module_priority}" -i "${script_dir}/kata-guest.cil"
# While the policy is being locked down, do not let base-policy dontaudit rules
# hide denials that kill a guest service. This can be removed after enforcing
# CI has exercised every supported workload.
semodule -n -DB

policy="${store_dir}/policy/policy.${policy_version}"
[[ -f "${policy}" ]] || { echo "expected ${policy} to be built" >&2; exit 1; }

# Fail the build rather than ship a policy the guest cannot use.
! sesearch "${policy}" --dontaudit | grep -q '^dontaudit ' \
	|| { echo "dontaudit rules remain in ${policy}" >&2; exit 1; }
for t in \
	kata_agent_t kata_coco_t kata_mkfs_t kata_nvidia_t \
	kata_scratch_device_t container_t container_file_t; do
	seinfo "${policy}" -t "${t}" | grep -q "${t}" \
		|| { echo "type ${t} missing from ${policy}" >&2; exit 1; }
done
sesearch "${policy}" -T -s kernel_t -t kata_agent_exec_t -c process | grep -q kata_agent_t \
	|| { echo "kernel_t -> kata_agent_t transition missing" >&2; exit 1; }
sesearch "${policy}" -T -s kata_coco_t -t fsadm_exec_t -c process | grep -q kata_mkfs_t \
	|| { echo "kata_coco_t -> kata_mkfs_t transition missing" >&2; exit 1; }
sesearch "${policy}" -T -s kata_coco_t -t device_t -c blk_file | grep -q kata_scratch_device_t \
	|| { echo "scratch block-device transition missing" >&2; exit 1; }
seinfo "${policy}" --constrain | grep -q \
	'constrain file { entrypoint execute_no_trans }.*kata_container_content' \
	|| { echo "container-content execution constraint missing" >&2; exit 1; }
seinfo "${policy}" --constrain | grep -q \
	'constrain file map.*kata_container_content.*kata_overlay_mounter' \
	|| { echo "container-content map constraint missing" >&2; exit 1; }
sesearch "${policy}" -T -s kata_agent_t -t container_runtime_tmpfs_t -c process | grep -q container_t \
	|| { echo "agent tmpfs exec -> container_t transition missing" >&2; exit 1; }
seinfo "${policy}" --constrain | grep -q \
	'constrain blk_file map.*kata_mkfs_t.*kata_scratch_device_t' \
	|| { echo "formatter block-map constraint missing" >&2; exit 1; }
# Every domain the guest runs is enforced. The base policy keeps its own
# permissive types, which nothing in the guest enters.
permissive="$(seinfo "${policy}" --permissive)"
for t in kernel_t kata_agent_t kata_coco_t kata_mkfs_t kata_nvidia_t container_t; do
	! grep -qw "${t}" <<< "${permissive}" \
		|| { echo "${t} is permissive in ${policy}" >&2; exit 1; }
done
seinfo "${policy}" | grep -q 'Handle unknown classes:[[:space:]]*deny' \
	|| { echo "${policy} must deny unknown permissions" >&2; exit 1; }
for perm in firmware_load kexec_image_load kexec_initramfs_load policy_load x509_certificate_load; do
	seinfo "${policy}" -c system -x | grep -qw "${perm}" \
		|| { echo "system ${perm} missing from ${policy}" >&2; exit 1; }
done
seinfo "${policy}" -c io_uring -x | grep -qw allowed \
	|| { echo "io_uring allowed missing from ${policy}" >&2; exit 1; }
# Every guest map must come from an explicit allow.
seinfo "${policy}" -b domain_can_mmap_files -x | grep -q 'domain_can_mmap_files false' \
	|| { echo "domain_can_mmap_files must default to false" >&2; exit 1; }
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
