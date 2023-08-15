#!/bin/bash
#
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ "${DEBUG}" != "" ]] && set -o xtrace
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../common.bash"

# runc is installed in /usr/local/sbin/ add that path
export PATH="$PATH:/usr/local/sbin"

# golang is installed in /usr/local/go/bin/ add that path
export PATH="$PATH:/usr/local/go/bin"

# Runtime to be used for testing
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
RUNTIME=${RUNTIME:-containerd-shim-kata-${KATA_HYPERVISOR}-v2}
FACTORY_TEST=${FACTORY_TEST:-""}
USE_DEVMAPPER="${USE_DEVMAPPER:-false}"
ARCH=$(uname -m)

containerd_runtime_type="io.containerd.kata-${KATA_HYPERVISOR}.v2"

containerd_shim_path="$(command -v containerd-shim)"

#containerd config file
readonly tmp_dir=$(mktemp -t -d test-cri-containerd.XXXX)
export REPORT_DIR="${tmp_dir}"
readonly CONTAINERD_CONFIG_FILE="${tmp_dir}/test-containerd-config"
readonly CONTAINERD_CONFIG_FILE_TEMP="${CONTAINERD_CONFIG_FILE}.temp"
readonly default_containerd_config="/etc/containerd/config.toml"
readonly default_containerd_config_backup="$CONTAINERD_CONFIG_FILE.backup"
readonly kata_config="/etc/kata-containers/configuration.toml"
readonly kata_config_backup="$kata_config.backup"
readonly default_kata_config="/opt/kata/share/defaults/kata-containers/configuration.toml"

function ci_config() {
	sudo mkdir -p $(dirname "${kata_config}")
	[ -f "$kata_config" ] && sudo cp "$kata_config" "$kata_config_backup" || \
		sudo cp "$default_kata_config" "$kata_config"

	source /etc/os-release || source /usr/lib/os-release
	ID=${ID:-""}
	if [ "$ID" == ubuntu ]; then
		# https://github.com/kata-containers/tests/issues/352
		if [ -n "${FACTORY_TEST}" ]; then
			sudo sed -i -e 's/^#enable_template.*$/enable_template = true/g' "${kata_config}"
			echo "init vm template"
			sudo -E PATH=$PATH "$RUNTIME" factory init
		fi
	fi

	echo "enable debug for kata-runtime"
	sudo sed -i 's/^#enable_debug =/enable_debug =/g' ${kata_config}
}

function ci_cleanup() {
	source /etc/os-release || source /usr/lib/os-release

	if [ -n "${FACTORY_TEST}" ]; then
		echo "destroy vm template"
		sudo -E PATH=$PATH "$RUNTIME" factory destroy
	fi

	if [ -e "$default_containerd_config_backup" ]; then
		echo "restore containerd config"
		sudo systemctl stop containerd
		sudo cp "$default_containerd_config_backup" "$default_containerd_config"
	fi

	[ -f "$kata_config_backup" ] && sudo mv "$kata_config_backup" "$kata_config" || \
		sudo rm "$kata_config"
}

function create_containerd_config() {
	local runtime="$1"
	# kata_annotations is set to 1 if caller want containerd setup with
	# kata annotations support.
	local kata_annotations=${2-0}
	[ -n "${runtime}" ] || die "need runtime to create config"

	local runtime_type="${containerd_runtime_type}"
	if [ "${runtime}" == "runc" ]; then
		runtime_type="io.containerd.runc.v2"
	fi
	local containerd_runtime=$(command -v "containerd-shim-${runtime}-v2")

cat << EOF | sudo tee "${CONTAINERD_CONFIG_FILE}"
[debug]
  level = "debug"
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
        default_runtime_name = "$runtime"
      [plugins.cri.containerd.runtimes.${runtime}]
        runtime_type = "${runtime_type}"
        $( [ $kata_annotations -eq 1 ] && \
        echo 'pod_annotations = ["io.katacontainers.*"]' && \
        echo '        container_annotations = ["io.katacontainers.*"]'
        )
        [plugins.cri.containerd.runtimes.${runtime}.options]
          Runtime = "${containerd_runtime}"
[plugins.linux]
       shim = "${containerd_shim_path}"
EOF

if [ "$USE_DEVMAPPER" == "true" ]; then
	sudo sed -i 's|^\(\[plugins\]\).*|\1\n  \[plugins.devmapper\]\n    pool_name = \"contd-thin-pool\"\n    base_image_size = \"4096MB\"|' ${CONTAINERD_CONFIG_FILE}
	echo "Devicemapper configured"
	cat "${CONTAINERD_CONFIG_FILE}"
fi

}

function cleanup() {
	ci_cleanup
	[ -d "$tmp_dir" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT

function err_report() {
	local log_file="${REPORT_DIR}/containerd.log"
	if [ -f "$log_file" ]; then
		echo "::group::ERROR: containerd log :"
		echo "-------------------------------------"
		cat "${log_file}"
		echo "-------------------------------------"
		echo "::endgroup::"
	fi
	echo "::group::ERROR: Kata Containers logs : "
	echo "-------------------------------------"
	sudo journalctl -xe -t kata
	echo "-------------------------------------"
	echo "::endgroup::"
}


function check_daemon_setup() {
	info "containerd(cri): Check daemon works with runc"
	create_containerd_config "runc"

	# containerd cri-integration will modify the passed in config file. Let's
	# give it a temp one.
	cp $CONTAINERD_CONFIG_FILE $CONTAINERD_CONFIG_FILE_TEMP
	# in some distros(AlibabaCloud), there is no btrfs-devel package available,
	# so pass GO_BUILDTAGS="no_btrfs" to make to not use btrfs.
	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="TestImageLoad" \
		RUNTIME="" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE_TEMP" \
		make GO_BUILDTAGS="no_btrfs" -e cri-integration
}

function testContainerStart() {
	# no_container_yaml set to 1 will not create container_yaml
	# because caller has created its own container_yaml.
	no_container_yaml=${1:-0}

	local pod_yaml=${REPORT_DIR}/pod.yaml
	local container_yaml=${REPORT_DIR}/container.yaml
	local image="busybox:latest"

	cat << EOF > "${pod_yaml}"
metadata:
  name: busybox-sandbox1
  namespace: default
  uid: busybox-sandbox1-uid
EOF

	#TestContainerSwap has created its own container_yaml.
	if [ $no_container_yaml -ne 1 ]; then
		cat << EOF > "${container_yaml}"
metadata:
  name: busybox-killed-vmm
  namespace: default
  uid: busybox-killed-vmm-uid
image:
  image: "$image"
command:
- top
EOF
	fi

	sudo cp "$default_containerd_config" "$default_containerd_config_backup"
	sudo cp $CONTAINERD_CONFIG_FILE "$default_containerd_config"

	restart_containerd_service

	sudo crictl pull $image
	podid=$(sudo crictl runp $pod_yaml)
	cid=$(sudo crictl create $podid $container_yaml $pod_yaml)
	sudo crictl start $cid
}

function testContainerStop() {
	info "stop pod $podid"
	sudo crictl stopp $podid
	info "remove pod $podid"
	sudo crictl rmp $podid

	sudo cp "$default_containerd_config_backup" "$default_containerd_config"
	restart_containerd_service
}

function TestKilledVmmCleanup() {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]]; then
		info "TestKilledVmmCleanup is skipped for ${KATA_HYPERVISOR}, only QEMU is currently tested"
		return 0
	fi

	info "test killed vmm cleanup"

	testContainerStart

	qemu_pid=$(ps aux|grep qemu|grep -v grep|awk '{print $2}')
	info "kill qemu $qemu_pid"
	sudo kill -SIGKILL $qemu_pid
	# sleep to let shimv2 exit
	sleep 1
	remained=$(ps aux|grep shimv2|grep -v grep || true)
	[ -z $remained ] || die "found remaining shimv2 process $remained"

	testContainerStop

	info "stop containerd"
}

function TestContainerMemoryUpdate() {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]] || [[ "${ARCH}" == "ppc64le" ]] || [[ "${ARCH}" == "s390x" ]]; then
		return
	fi

	test_virtio_mem=$1

	if [ $test_virtio_mem -eq 1 ]; then
		if [[ "$ARCH" != "x86_64" ]]; then
			return
		fi
		info "Test container memory update with virtio-mem"

		sudo sed -i -e 's/^#enable_virtio_mem.*$/enable_virtio_mem = true/g' "${kata_config}"
	else
		info "Test container memory update without virtio-mem"

		sudo sed -i -e 's/^enable_virtio_mem.*$/#enable_virtio_mem = true/g' "${kata_config}"
	fi

	testContainerStart

	vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
	if [ $vm_size -gt $((2*1024*1024*1024)) ] || [ $vm_size -lt $((2*1024*1024*1024-128*1024*1024)) ]; then
		testContainerStop
		die "The VM memory size $vm_size before update is not right"
	fi

	sudo crictl update --memory $((2*1024*1024*1024)) $cid
	sleep 1

	vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
	if [ $vm_size -gt $((4*1024*1024*1024)) ] || [ $vm_size -lt $((4*1024*1024*1024-128*1024*1024)) ]; then
		testContainerStop
		die "The VM memory size $vm_size after increase is not right"
	fi

	if [ $test_virtio_mem -eq 1 ]; then
		sudo crictl update --memory $((1*1024*1024*1024)) $cid
		sleep 1

		vm_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "MemTotal:" | awk '{print $2}')*1024))
		if [ $vm_size -gt $((3*1024*1024*1024)) ] || [ $vm_size -lt $((3*1024*1024*1024-128*1024*1024)) ]; then
			testContainerStop
			die "The VM memory size $vm_size after decrease is not right"
		fi
	fi

	testContainerStop
}

function getContainerSwapInfo() {
	swap_size=$(($(sudo crictl exec $cid cat /proc/meminfo | grep "SwapTotal:" | awk '{print $2}')*1024))
	# NOTE: these below two checks only works on cgroup v1
	swappiness=$(sudo crictl exec $cid cat /sys/fs/cgroup/memory/memory.swappiness)
	swap_in_bytes=$(sudo crictl exec $cid cat /sys/fs/cgroup/memory/memory.memsw.limit_in_bytes)
}

function TestContainerSwap() {
	if [[ "${KATA_HYPERVISOR}" != "qemu" ]] || [[ "${ARCH}" != "x86_64" ]]; then
		return
	fi

	local container_yaml=${REPORT_DIR}/container.yaml
	local image="busybox:latest"

	info "Test container with guest swap"

	create_containerd_config "kata-${KATA_HYPERVISOR}" 1
	sudo sed -i -e 's/^#enable_guest_swap.*$/enable_guest_swap = true/g' "${kata_config}"

	# Test without swap device
	testContainerStart
	getContainerSwapInfo
	# Current default swappiness is 60
	if [ $swappiness -ne 60 ]; then
		testContainerStop
		die "The VM swappiness $swappiness without swap device is not right"
	fi
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		testContainerStop
		die "The VM swap_in_bytes $swap_in_bytes without swap device is not right"
	fi
	if [ $swap_size -ne 0 ]; then
		testContainerStop
		die "The VM swap size $swap_size without swap device is not right"
	fi
	testContainerStop

	# Test with swap device
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
  io.katacontainers.container.resource.swap_in_bytes: "1610612736"
linux:
  resources:
    memory_limit_in_bytes: 1073741824
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo
	testContainerStop

	if [ $swappiness -ne 100 ]; then
		die "The VM swappiness $swappiness with swap device is not right"
	fi
	if [ $swap_in_bytes -ne 1610612736 ]; then
		die "The VM swap_in_bytes $swap_in_bytes with swap device is not right"
	fi
	if [ $swap_size -ne 536870912 ]; then
		die "The VM swap size $swap_size with swap device is not right"
	fi

	# Test without swap_in_bytes
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
linux:
  resources:
    memory_limit_in_bytes: 1073741824
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo
	testContainerStop

	if [ $swappiness -ne 100 ]; then
		die "The VM swappiness $swappiness without swap_in_bytes is not right"
	fi
	# swap_in_bytes is not set, it should be a value that bigger than 1125899906842624
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		die "The VM swap_in_bytes $swap_in_bytes without swap_in_bytes is not right"
	fi
	if [ $swap_size -ne 1073741824 ]; then
		die "The VM swap size $swap_size without swap_in_bytes is not right"
	fi

	# Test without memory_limit_in_bytes
	cat << EOF > "${container_yaml}"
metadata:
  name: busybox-swap
  namespace: default
  uid: busybox-swap-uid
annotations:
  io.katacontainers.container.resource.swappiness: "100"
image:
  image: "$image"
command:
- top
EOF

	testContainerStart 1
	getContainerSwapInfo
	testContainerStop

	if [ $swappiness -ne 100 ]; then
		die "The VM swappiness $swappiness without memory_limit_in_bytes is not right"
	fi
	# swap_in_bytes is not set, it should be a value that bigger than 1125899906842624
	if [ $swap_in_bytes -lt 1125899906842624 ]; then
		die "The VM swap_in_bytes $swap_in_bytes without memory_limit_in_bytes is not right"
	fi
	if [ $swap_size -ne 2147483648 ]; then
		die "The VM swap size $swap_size without memory_limit_in_bytes is not right"
	fi

	create_containerd_config "kata-${KATA_HYPERVISOR}"
}

# k8s may restart docker which will impact on containerd stop
function stop_containerd() {
	local tmp=$(pgrep kubelet || true)
	[ -n "$tmp" ] && sudo kubeadm reset -f

	sudo systemctl stop containerd
}

function main() {

	info "Stop crio service"
	systemctl is-active --quiet crio && sudo systemctl stop crio

	info "Stop containerd service"
	systemctl is-active --quiet containerd && stop_containerd

	# Configure enviroment if running in CI
	ci_config

	pushd "containerd"

	# Make sure the right artifacts are going to be built
	make clean

	check_daemon_setup

	info "containerd(cri): testing using runtime: ${containerd_runtime_type}"

	create_containerd_config "kata-${KATA_HYPERVISOR}"

	info "containerd(cri): Running cri-integration"


	passing_test="TestContainerStats|TestContainerRestart|TestContainerListStatsWithIdFilter|TestContainerListStatsWithIdSandboxIdFilter|TestDuplicateName|TestImageLoad|TestImageFSInfo|TestSandboxCleanRemove"

	if [[ "${KATA_HYPERVISOR}" == "cloud-hypervisor" || \
		"${KATA_HYPERVISOR}" == "qemu" ]]; then
		issue="https://github.com/kata-containers/tests/issues/2318"
		info "${KATA_HYPERVISOR} fails with TestContainerListStatsWithSandboxIdFilter }"
		info "see ${issue}"
	else
		passing_test="${passing_test}|TestContainerListStatsWithSandboxIdFilter"
	fi

	# in some distros(AlibabaCloud), there is no btrfs-devel package available,
	# so pass GO_BUILDTAGS="no_btrfs" to make to not use btrfs.
	# containerd cri-integration will modify the passed in config file. Let's
	# give it a temp one.
	cp $CONTAINERD_CONFIG_FILE $CONTAINERD_CONFIG_FILE_TEMP
	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="^(${passing_test})$" \
		RUNTIME="" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE_TEMP" \
		make GO_BUILDTAGS="no_btrfs" -e cri-integration

	# trap error for print containerd log,
	# containerd's `cri-integration` will print the log itself.
	trap err_report ERR

	# TestContainerSwap is currently failing with GHA.
	# Let's re-enable it as soon as we get it to work.
	# Reference: https://github.com/kata-containers/kata-containers/issues/7410
	# TestContainerSwap

	# TODO: runtime-rs doesn't support memory update currently
	if [ "$KATA_HYPERVISOR" != "dragonball" ]; then
		TestContainerMemoryUpdate 1
		TestContainerMemoryUpdate 0
	fi

	TestKilledVmmCleanup

	popd
}

main
