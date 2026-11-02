#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers
#
# SPDX-License-Identifier: Apache-2.0
#
# Boot a QEMU guest configured with both an image-backed rootfs and an initrd.
#
# The two assets used to be mutually exclusive for every hypervisor. QEMU
# accepts them together so that a bootstrap initrd can bring up the devices the
# minimal guest needs and then mount the image as the rootfs.
#
# The initrd shipped by osbuilder boots from the initramfs instead of mounting
# the image, so this test covers the runtime side: both assets are accepted for
# one VM, `root=` is still emitted, and the initrd reaches the VMM.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

kata_image_path="/opt/kata/share/kata-containers/kata-containers.img"
kata_initrd_path="/opt/kata/share/kata-containers/kata-containers-initrd.img"

# Returns 0 if the current environment supports booting from an image and an
# initrd at the same time, non-zero otherwise. Only QEMU allows the
# combination, and the confidential classes already boot a measured rootfs
# through their own verity-aware initrd.
image_and_initrd_supported() {
	case "${KATA_HYPERVISOR}" in
		qemu | qemu-runtime-rs) ;;
		*) return 1 ;;
	esac

	is_confidential_runtime_class && return 1
	return 0
}

setup() {
	if ! image_and_initrd_supported; then
		skip "image and initrd together are a QEMU-only, non-confidential feature (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	fi

	setup_common || die "setup_common failed"

	# Both assets must exist on the node: without the initrd this would be a
	# plain image boot, which is already covered by every other test.
	exec_host "$node" "test -f ${kata_image_path} && test -f ${kata_initrd_path} && echo present" | grep -q present \
		|| skip "missing ${kata_image_path} or ${kata_initrd_path} on node ${node}"

	# A drop-in in config.d is read after the main configuration, so setting
	# both keys here overrides the image-only default.
	local runtime_config_dropin_file="${BATS_TEST_TMPDIR}/99-image-and-initrd.toml"
	cat > "${runtime_config_dropin_file}" <<DROPIN
[hypervisor.qemu]
image = "${kata_image_path}"
initrd = "${kata_initrd_path}"
DROPIN

	dropin_path="$(set_kata_runtime_config_dropin_file "$node" "${runtime_config_dropin_file}")" \
		|| die "Failed to install Kata runtime config drop-in on node $node"

	yaml_file="${pod_config_dir}/image-and-initrd.yaml"
	set_node "${yaml_file}" "${node}" \
		|| die "Failed to pin the pod to node $node"
}

@test "QEMU guest boots with both an image and an initrd configured" {
	pod_name="image-and-initrd"

	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="$timeout" "pod/${pod_name}"

	# A guest that runs the workload proves the configuration was accepted and
	# the VM booted; a runtime that still rejects the pair fails the wait above.
	grep_pod_exec_output "${pod_name}" "image-and-initrd-ok" \
		sh -c "echo 'image-and-initrd-ok'"

	# `root=` is emitted only for an image-backed rootfs, and it has to survive
	# the initrd being set: the initrd is a bootstrap stage, not a replacement
	# for the image. An initrd also has to move the image off NVDIMM, and
	# runtime-rs defaults to virtio-pmem on x86_64, so the device name is what
	# covers the move to virtio-blk.
	grep_pod_exec_output "${pod_name}" "root=/dev/vda1" cat /proc/cmdline

	# The initrd has to reach the VMM as well. Asserting on the host command line
	# is the only place the two assets are both visible, since a guest booted
	# from the stock initrd cannot show which device it declined to mount.
	if ! exec_host "${node}" "pgrep -af 'kata-containers-initrd[.]img' > /dev/null && echo passed" | grep -q passed; then
		die "no VMM on node $node was started with ${kata_initrd_path}"
	fi
}

teardown() {
	image_and_initrd_supported || return 0

	# A leaked drop-in would silently apply the combination to every later pod
	# on the same node.
	remove_kata_runtime_config_dropin_file "$node" "${dropin_path:-}" \
		|| echo "Warning: Failed to remove Kata runtime config drop-in on node $node"

	teardown_common "${node:-}" "${node_start_time:-}"
}
