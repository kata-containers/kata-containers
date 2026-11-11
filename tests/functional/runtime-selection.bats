#!/usr/bin/env bats
# Copyright (c) 2026 CoreWeave Inc.
#
# SPDX-License-Identifier: Apache-2.0

# Bats supplies the globals and isolates each test in its own subshell.
# shellcheck disable=SC2154,SC2030,SC2031

load "${BATS_TEST_DIRNAME}/../common.bash"

setup() {
	commands="${BATS_TEST_TMPDIR}/commands"
	export commands
	# Called indirectly by the loaded installation helpers.
	# shellcheck disable=SC2317
	sudo() { printf '%s\n' "$*" >> "${commands}"; }
	export -f sudo
}

# These callbacks intercept host installation operations in install_kata.
# shellcheck disable=SC2317
mock_install() {
	install_tarball() { printf '%s\n' "$3" >> "${commands}"; }
	link_binaries() { :; }
	check_containerd_config_for_kata() { :; }
	restart_containerd_service() { :; }
	load_vhost_mods() { :; }
	mkdir "${BATS_TEST_TMPDIR}/kata-artifacts"
	touch "${BATS_TEST_TMPDIR}/kata-artifacts/kata-go-static.tar.zst"
	cd "${BATS_TEST_TMPDIR}" || return
}

@test "Go Firecracker installs the Go artifact when both artifacts are available" {
	mock_install
	KATA_HYPERVISOR="fc" install_kata
	grep -Fx kata-go-static.tar.zst "${commands}"
}

@test "Rust Firecracker installs the Rust artifact when the Go artifact is available" {
	mock_install
	KATA_HYPERVISOR="rs-fc" install_kata
	grep -Fx kata-static.tar.zst "${commands}"
}

@test "Go Firecracker selects its shim and configuration" {
	export KATA_HYPERVISOR=fc
	enabling_hypervisor
	grep -Fx 'ln -sf /opt/kata/bin/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-fc-v2' "${commands}"
	[[ "${KATA_CONFIG_PATH}" == /opt/kata/share/defaults/kata-containers/configuration.toml ]]
	grep -F '/configuration-fc.toml ' "${commands}"
}

@test "Rust Firecracker selects its shim and configuration" {
	export KATA_HYPERVISOR=rs-fc
	enabling_hypervisor
	grep -Fx 'ln -sf /opt/kata/runtime-rs/bin/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-rs-fc-v2' "${commands}"
	[[ "${KATA_CONFIG_PATH}" == /opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml ]]
	grep -F '/configuration-rs-fc.toml ' "${commands}"
}

@test "Rust runtime names retain their classification" {
	local runtime
	for runtime in rs-fc dragonball qemu-runtime-rs clh-runtime-rs; do
		is_rust_hypervisor "${runtime}"
	done
	for runtime in fc firecracker qemu clh; do
		! is_rust_hypervisor "${runtime}"
	done
}

@test "All Firecracker names inherit the existing hotplug exclusion" {
	local runtime
	for runtime in fc firecracker rs-fc; do
		is_firecracker_hypervisor "${runtime}"
		! is_hotplug_supported "${runtime}"
	done
}
