#!/bin/bash
#
# Copyright (c) 2023 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0
#

[ -n "$DEBUG" ] && set -x
set -o nounset
set -o pipefail
set -o errtrace

script_path=$(dirname "$0")
registry_port="${REGISTRY_PORT:-5000}"
registry_name="local-registry"
container_engine="${container_engine:-docker}"
dev_base="/dev/vfio"
sys_bus_base="/sys/bus/ap"
sys_device_base="/sys/devices/vfio_ap/matrix"
command_file="mdev_supported_types/vfio_ap-passthrough/create"
test_image_name="localhost:${registry_port}/vfio-ap-test:latest"
kata_base="/opt/kata"
runtime_config_base="${kata_base}/share/defaults/kata-containers"

test_category="[kata][vfio-ap][containerd]"

trap cleanup EXIT

# Prevent the program from exiting on error
trap - ERR
registry_image="registry:2.8.3"

setup_config_file() {
    local target_item=$1
    local action=$2
    local replacement=${3:-}
    local runtime_dir=${4:-}
    local kata_config_file="${runtime_config_base}/${runtime_dir}/configuration.toml"

    if [ "${action}" == "comment_out" ]; then
        sudo sed -i -e 's/.*\('${target_item}'.*\)/#\1/g' ${kata_config_file}
    else
        sudo sed -i -e 's/.*\('${target_item}'\s*=\).*/\1 "'${replacement}'"/g' ${kata_config_file}
    fi
}

show_config_file() {
    local runtime_dir=${1:-}
    local kata_config_file="${runtime_config_base}/${runtime_dir}/configuration.toml"
    echo "Show kata configuration"
    # Print out the configuration by excluding comments and empty lines
    cat "${kata_config_file}" | grep -v '^\s*$\|^\s*\#'
}

setup_hotplug() {
    local runtime=$1
    echo "Set up the configuration file for Hotplug for ${runtime}"
    if [ "${runtime}" == "runtime" ]; then
        setup_config_file "vfio_mode" "replace" "vfio"
        setup_config_file "cold_plug_vfio" "comment_out"
        setup_config_file "hot_plug_vfio" "replace" "bridge-port"
        show_config_file
    elif [ "${runtime}" == "runtime-rs" ]; then
        setup_config_file "vfio_mode" "replace" "vfio" "runtime-rs"
        show_config_file "runtime-rs"
    else
        echo "Invalid runtime: ${runtime}" >&2
        exit 1
    fi
}

setup_coldplug() {
    local runtime=$1
    echo "Set up the configuration file for Coldplug for ${runtime}"
    if [ "${runtime}" == "runtime" ]; then
        setup_config_file "vfio_mode" "replace" "vfio"
        setup_config_file "hot_plug_vfio" "comment_out"
        setup_config_file "cold_plug_vfio" "replace" "bridge-port"
        show_config_file
    elif [ "${runtime}" == "runtime-rs" ]; then
        echo "Coldplug is not supported for runtime-rs" >&2
        exit 1
    else
        echo "Invalid runtime: ${runtime}" >&2
        exit 1
    fi
}

cleanup() {
    # Clean up ctr resources
    sudo ctr image rm $(sudo ctr image list -q) || true

    # Remove the test image
    ${container_engine} rmi -f ${test_image_name} > /dev/null 2>&1

    # Destroy mediated devices
    IFS=$'\n' read -r -d '' -a arr_dev < <( ls -1 /sys/bus/mdev/devices && printf '\0' )
    for item in "${arr_dev[@]}"; do
        if [[ ${item//-/} =~ ^[[:xdigit:]]{32}$ ]]; then
            echo 1 | sudo tee /sys/bus/mdev/devices/${item}/remove > /dev/null
        fi
    done

    # Release devices from vfio-ap
    echo 0x$(printf -- 'f%.0s' {1..64}) | sudo tee /sys/bus/ap/apmask > /dev/null
    echo 0x$(printf -- 'f%.0s' {1..64}) | sudo tee /sys/bus/ap/aqmask > /dev/null

    # Remove files used for testing
    rm -f ${script_path}/zcrypttest
}

validate_env() {
    if [ ! -f ${HOME}/script/zcrypttest ]; then
        echo "zcrypttest not found" >&2
        exit 1
    fi
    necessary_commands=( "${container_engine}" "ctr" "lszcrypt" )
    for cmd in "${necessary_commands[@]}"; do
        if ! which ${cmd} > /dev/null 2>&1; then
            echo "${cmd} not found" >&2
            exit 1
        fi
    done

    if ! ${container_engine} ps | grep -q "${registry_name}"; then
        echo "Docker registry not found. Installing..."
        ${container_engine} run -d -p ${registry_port}:5000 --restart=always --name "${registry_name}" "${registry_image}"
        # wait for registry container
        waitForProcess 15 3 "curl http://localhost:${registry_port}"
    fi

    sudo modprobe vfio
    sudo modprobe vfio_ap
}

build_test_image() {
    cp ${HOME}/script/zcrypttest ${script_path}
    ${container_engine} rmi -f ${test_image_name} > /dev/null 2>&1
    ${container_engine} build --no-cache -t ${test_image_name} ${script_path}
    ${container_engine} push ${test_image_name}
}

create_mediated_device() {
    # a device lastly listed is chosen
    APQN=$(lszcrypt | tail -1 | awk '{ print $1}')
    if [[ ! $APQN =~ [[:xdigit:]]{2}.[[:xdigit:]]{4} ]]; then
        echo "Incorrect format for APQN" >&2
        exit 1
    fi
    _APID=${APQN//.*}
    _APQI=${APQN#*.}
    APID=$(echo ${_APID} | sed 's/^0*//')
    APID=${APID:-0}
    APQI=$(echo ${_APQI} | sed 's/^0*//')
    APQI=${APQI:-0}

    # Release the device from the host
    pushd ${sys_bus_base}
    echo -0x${APID} | sudo tee apmask
    echo -0x${APQI} | sudo tee aqmask
    popd
    lszcrypt --verbose

    # Create a mediated device (mdev) for the released device
    echo "Status before creation of  mediated device"
    ls ${dev_base}

    pushd ${sys_device_base}
    if [ ! -f ${command_file} ]; then
        echo "${command_file} not found}" >&2
        exit 1
    fi

    mdev_uuid=$(uuidgen)
    echo "${mdev_uuid}" | sudo tee ${command_file}

    echo "Status after creation of mediated device"
    ls ${dev_base}

    [ -n "${mdev_uuid}" ] && cd ${mdev_uuid}
    if [ ! -L iommu_group ]; then
        echo "${mdev_uuid}/iommu_group not found" >&2
        exit 1
    fi
    dev_index=$(readlink iommu_group | xargs -i{} basename {})
    if [ ! -n "${dev_index}" ]; then
        echo "No dev_index from 'readlink ${sys_device_base}/${mdev_uuid}/iommu_group'" >&2
        exit 1
    fi
    cat matrix
    echo 0x${APID} | sudo tee assign_adapter
    echo 0x${APQI} | sudo tee assign_domain
    cat matrix
    popd
}

run_test() {
    local run_index=$1
    local runtime=$2
    local test_message=$3
    local extra_cmd=${4:-}
    local start_time=$(date +"%Y-%m-%d %H:%M:%S")
    [ -n "${dev_index}" ] || { echo "No dev_index" >&2; exit 1; }

    if [ "${runtime}" == "runtime" ]; then
        local runtime_type="io.containerd.run.kata.v2"
    elif [ "${runtime}" == "runtime-rs" ]; then
        local runtime_type="io.containerd.kata-qemu-runtime-rs.v2"
    else
        echo "Invalid runtime: ${runtime}" >&2
        exit 1
    fi

    # Set time granularity to a second for capturing the log
    sleep 1

    sudo ctr image pull --plain-http ${test_image_name}
    # Create a container and run the test
    sudo ctr run --runtime ${runtime_type} --rm \
        --privileged --privileged-without-host-devices \
        --device ${dev_base}/${dev_index} ${test_image_name} test \
        bash -c "lszcrypt ${_APID}.${_APQI} | grep ${APQN} ${extra_cmd}"

    [ $? -eq 0 ] && result=0 || result=1

    if [ $result -eq 0 ]; then
        echo "ok ${run_index} ${test_category}[${runtime}] ${test_message}"
    else
        echo "not ok ${run_index} ${test_category}[${runtime}] ${test_message}"
        echo "Logging the journal..."
        sudo journalctl --no-pager --since "${start_time}"
    fi
}

configure_containerd_for_runtime_rs() {
    local config_file="/etc/containerd/config.toml"

    sudo rm -f /usr/local/bin/containerd-shim-kata-qemu-runtime-rs-v2 \
        ${runtime_config_base}/runtime-rs/configuration.toml
    if [ ! -f ${kata_base}/runtime-rs/bin/containerd-shim-kata-v2 ]; then
        echo "${kata_base}/runtime-rs/bin/containerd-shim-kata-v2 not found" >&2
        exit 1
    fi
    if [ ! -f ${runtime_config_base}/runtime-rs/configuration-qemu-runtime-rs.toml ]; then
        echo "${runtime_config_base}/runtime-rs/configuration-qemu-runtime-rs.toml not found" >&2
        exit 1
    fi
    sudo ln -sf ${kata_base}/runtime-rs/bin/containerd-shim-kata-v2 \
        /usr/local/bin/containerd-shim-kata-qemu-runtime-rs-v2
    sudo ln -sf ${runtime_config_base}/runtime-rs/configuration-qemu-runtime-rs.toml \
        ${runtime_config_base}/runtime-rs/configuration.toml

    if [ ! -f ${config_file} ]; then
        echo "/etc/containerd/config.toml not found" >&2
        exit 1
    fi

    if ! grep -q "kata-qemu-runtime-rs" ${config_file}; then
        cat <<EOF | sudo tee -a ${config_file}
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata-qemu-runtime-rs]
          runtime_type = "io.containerd.kata-qemu-runtime-rs.v2"
EOF
    fi

    sudo systemctl daemon-reload
    sudo systemctl restart containerd
    # Wait for containerd to restart and the configuration to take effect
    sleep 1
}

run_tests() {
    setup_hotplug "runtime"
    run_test "1" "runtime" "Test can assign a CEX device inside the guest via VFIO-AP Hotplug" "&& zcrypttest -a -v"

    setup_coldplug "runtime"
    run_test "2" "runtime" "Test can assign a CEX device inside the guest via VFIO-AP Coldplug" "&& zcrypttest -a -v"

    configure_containerd_for_runtime_rs

    setup_hotplug "runtime-rs"
    run_test "3" "runtime-rs" "Test can assign a CEX device inside the guest via VFIO-AP Hotplug" "&& zcrypttest -a -v"
}

main() {
    validate_env
    cleanup
    build_test_image
    create_mediated_device
    run_tests
}

main "$@"
