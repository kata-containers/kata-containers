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
source "${script_path}/../../lib/common.bash"

registry_port="${REGISTRY_PORT:-5000}"
registry_name="kata-registry"
container_engine="${container_engine:-docker}"
dev_base="/dev/vfio"
sys_bus_base="/sys/bus/ap"
sys_device_base="/sys/devices/vfio_ap/matrix"
command_file="mdev_supported_types/vfio_ap-passthrough/create"
test_image_name="localhost:${registry_port}/vfio-ap-test:latest"

test_category="[kata][vfio-ap][containerd]"

trap cleanup EXIT

# Check if the given function exists.
function_exists() {
    [[ "$(type -t $1)" == "function" ]]
}

if ! function_exists get_test_version; then
    source "${script_path}/../../.ci/lib.sh"
fi

# Prevent the program from exiting on error
trap - ERR
image_version=$(get_test_version "docker_images.registry_ibm.version")
registry_image=$(get_test_version "docker_images.registry_ibm.registry_url"):"${image_version}"

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
    setup_config_file "vfio_mode" "replace" "guest-kernel"
    setup_config_file "hot_plug_vfio" "comment_out"
    setup_config_file "cold_plug_vfio" "replace" "bridge-port"
    show_config_file
}

cleanup() {
    # Clean up container images
    sudo ctr image rm $(sudo ctr image list -q) || :
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
}

validate_env() {
    necessary_commands=( "${container_engine}" "ctr" "lszcrypt" )
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

    sudo modprobe vfio
    sudo modprobe vfio_ap
}

build_test_image() {
    ${container_engine} rmi -f ${test_image_name} > /dev/null 2>&1
    ${container_engine} build -t ${test_image_name} ${script_path}
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
    APQI=$(echo ${_APQI} | sed 's/^0*//')

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
    local start_time=$(date +"%Y-%m-%d %H:%M:%S")
    # Set time granularity to a second for capturing the log
    sleep 1
    # Check if the APQN is identified in a container
    sudo ctr image pull --plain-http ${test_image_name}

    [ -n "${dev_index}" ] && \
        sudo ctr run --runtime io.containerd.run.kata.v2 --rm \
        --device ${dev_base}/${dev_index} ${test_image_name} test \
        bash -c "lszcrypt ${_APID}.${_APQI} | grep ${APQN}"
    if [ $? -eq 0 ]; then
        echo "ok ${run_index} ${test_category} ${test_message}"
    else
        echo "not ok ${run_index} ${test_category} ${test_message}"
        echo "Logging the journal..."
        sudo journalctl --no-pager --since "${start_time}"
    fi
}

run_tests() {
    setup_hotplug
    run_test "1" "Test can assign a CEX device inside the guest via VFIO-AP Hotplug"

    setup_coldplug
    run_test "2" "Test can assign a CEX device inside the guest via VFIO-AP Coldplug"
}

main() {
    validate_env
    cleanup
    build_test_image
    create_mediated_device
    run_tests
}

main $@
