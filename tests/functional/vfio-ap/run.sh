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

test_category="[kata][vfio-ap][containerd]"

trap cleanup EXIT

# Prevent the program from exiting on error
trap - ERR
registry_image="registry:2.8.3"

setup_config_file() {
    local target_item=$1
    local action=$2
    local replacement=${3:-}
    local kata_config_file=$(kata-runtime env --json | jq -r '.Runtime.Config.Path')

    if [ "${action}" == "comment_out" ]; then
        sudo sed -i -e 's/.*\('${target_item}'.*\)/#\1/g' ${kata_config_file}
    else
        sudo sed -i -e 's/.*\('${target_item}'\s*=\).*/\1 "'${replacement}'"/g' ${kata_config_file}
    fi
}

show_config_file() {
    local kata_config_file=$(kata-runtime env --json | jq -r '.Runtime.Config.Path')
    echo "Show kata configuration"
    # Print out the configuration by excluding comments and empty lines
    cat "${kata_config_file}" | grep -v '^\s*$\|^\s*\#'
}

setup_hotplug() {
    echo "Set up the configuration file for Hotplug"
    setup_config_file "vfio_mode" "replace" "vfio"
    setup_config_file "cold_plug_vfio" "comment_out"
    setup_config_file "hot_plug_vfio" "replace" "bridge-port"
    show_config_file
}

setup_coldplug() {
    echo "Set up the configuration file for Coldplug"
    setup_config_file "vfio_mode" "replace" "vfio"
    setup_config_file "hot_plug_vfio" "comment_out"
    setup_config_file "cold_plug_vfio" "replace" "bridge-port"
    show_config_file
}

cleanup() {
    # Clean up ctr resources
    sudo ctr image rm $(sudo ctr image list -q) || true

    # Clean up crictl resources
    for pod_id in $(sudo crictl pods -q); do
        sudo crictl stopp $pod_id
        sudo crictl rmp $pod_id
    done
    sudo crictl rmi $(sudo crictl images -q) || true

    # Remove the test image
    ${container_engine} rmi -f ${test_image_name} > /dev/null 2>&1

    # Destroy mediated devices
    IFS=$'\n' read -r -d '' -a arr_dev < <( ls -1 /sys/bus/mdev/devices && printf '\0' )
    for item in ${arr_dev[@]}; do
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
    necessary_commands=( "${container_engine}" "ctr" "crictl" "lszcrypt" )
    for cmd in ${necessary_commands[@]}; do
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

    # Check if /etc/containerd/config.toml containers `privileged_without_host_devices = true`
    if [ -f /etc/containerd/config.toml ]; then
        if ! grep -q "privileged_without_host_devices *= *true" /etc/containerd/config.toml; then
            echo "privileged_without_host_devices = true not found in /etc/containerd/config.toml"
            echo "Adding it..."
            local runtime_type='runtime_type *= *"io.containerd.kata.v2"'
            local new_line='privileged_without_host_devices = true'
            local file_path='/etc/containerd/config.toml'
            # Find a line with a pattern runtime_type and duplicate it
            sudo sed -i "/$runtime_type/{h;G}" "$file_path"
            # Replace the duplicated line with new_line
            sudo sed -i "/$runtime_type/{n;s/^\([[:space:]]*\).*/\1$new_line/;}" "$file_path"
            # Restart containerd
            sudo systemctl daemon-reload
            sudo systemctl restart containerd
        fi
    else
        echo "/etc/containerd/config.toml not found" >&2
        exit 1
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
    local test_message=$2
    local extra_cmd=${3:-}
    local start_time=$(date +"%Y-%m-%d %H:%M:%S")
    [ -n "${dev_index}" ] || { echo "No dev_index" >&2; exit 1; }

    # Set time granularity to a second for capturing the log
    sleep 1

    sudo ctr image pull --plain-http ${test_image_name}
    # Create a container and run the test
    sudo ctr run --runtime io.containerd.run.kata.v2 --rm \
        --privileged --privileged-without-host-devices \
        --device ${dev_base}/${dev_index} ${test_image_name} test \
        bash -c "lszcrypt ${_APID}.${_APQI} | grep ${APQN} ${extra_cmd}"

    [ $? -eq 0 ] && result=0 || result=1

    if [ $result -eq 0 ]; then
        echo "ok ${run_index} ${test_category} ${test_message}"
    else
        echo "not ok ${run_index} ${test_category} ${test_message}"
        echo "Logging the journal..."
        sudo journalctl --no-pager --since "${start_time}"
    fi
}

run_tests() {
    setup_hotplug
    run_test "1" "Test can assign a CEX device inside the guest via VFIO-AP Hotplug" "&& zcrypttest -a -v"

    setup_coldplug
    run_test "2" "Test can assign a CEX device inside the guest via VFIO-AP Coldplug" "&& zcrypttest -a -v"
}

main() {
    validate_env
    cleanup
    build_test_image
    create_mediated_device
    run_tests
}

main $@
