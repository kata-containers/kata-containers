#!/bin/bash -e
#
# Copyright (c) 2021 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Disclaimer: This script is work in progress for supporting the CCv0 prototype
# It shouldn't be considered supported by the Kata Containers community, or anyone else

# Based on https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md, but with elements of the tests/.ci scripts used

readonly script_name="$(basename "${BASH_SOURCE[0]}")"

# By default in Golang >= 1.16 GO111MODULE is set to "on", but not all modules support it, so overwrite to "auto"
export GO111MODULE="auto"

# Setup kata containers environments if not set - we default to use containerd
export CRI_CONTAINERD=${CRI_CONTAINERD:-"yes"}
export CRI_RUNTIME=${CRI_RUNTIME:-"containerd"}
export CRIO=${CRIO:-"no"}
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export KUBERNETES=${KUBERNETES:-"yes"}
export AGENT_INIT="${AGENT_INIT:-${TEST_INITRD:-no}}"

# Allow the user to overwrite the default repo and branch names if they want to build from a fork
export katacontainers_repo="${katacontainers_repo:-github.com/kata-containers/kata-containers}"
export katacontainers_branch="${katacontainers_branch:-CCv0}" 
export kata_default_branch=${katacontainers_branch}
export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_branch="${tests_branch:-CCv0}"
export target_branch=${tests_branch} # kata-containers/ci/lib.sh uses target branch var to check out tests repo

# Create a bunch of common, derived values up front so we don't need to create them in all the different functions
. "$HOME/.profile"
if [ -z ${GOPATH} ]; then
    export GOPATH=${HOME}/go
fi
export tests_repo_dir="${GOPATH}/src/${tests_repo}"
export katacontainers_repo_dir="${GOPATH}/src/${katacontainers_repo}"
export ROOTFS_DIR="${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder/rootfs"
export PULL_IMAGE="${PULL_IMAGE:-registry.fedoraproject.org/fedora:latest}" # Doesn't need authentication
export CONTAINER_ID="${CONTAINER_ID:-0123456789}"

debug_output() {
    if [ -n "${DEBUG}" ]
    then
        echo "$(date): $@"
    fi
}

debug_function() {
    debug_output "> $@"
    start=$(date +%s%N | cut -b1-13)
    $@;
    status=$?
    end=$(date +%s%N | cut -b1-13)
    time=`expr ${end} - ${start}`
    debug_output "< $@. Time taken: $(echo "scale=2; ${time} / 1000" | bc -l)s. RC: ${status}"
}

usage() {
    exit_code="$1"
    cat <<EOT
Overview:
    Build and test kata containers from source
    Optionally set kata-containers and tests repo and branch as exported variables before running
    e.g. export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=kata-ci-from-fork && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=kata-ci-from-fork && . ~/${script_name} -d build_and_install_all
Usage:
    ${script_name} [options] <command>
Commands:
- help:                         Display this help
- all:                          Build and install everything, test kata with containerd and capture the logs
- build_and_install_all:        Build and install everything
- initialize:                   Install dependencies and check out kata-containers source
- rebuild_and_install_kata:     Rebuild the kata runtime and agent and build and install the image
- build_kata_runtime:           Build and install the kata runtime
- configure:                    Configure Kata to use rootfs and enable debug
- create_rootfs:                Create a local rootfs
- build_and_add_agent_to_rootfs:Builds the kata-agent and adds it to the rootfs
- build_and_install_rootfs:     Builds and installs the rootfs image
- install_guest_kernel:         Setup, build and install the guest kernel
- build_qemu:                   Checkout, patch, build and install QEMU
- init_kubernetes:              initialize a Kubernetes cluster on this system
- create_kata_pod:              Create a kata runtime nginx pod in Kubernetes
- delete_kata_pod:              Delete a kata runtime nginx pod in Kubernetes
- restart_kata_pod:             Delete the kata nginx pod, then re-create it
- open_kata_console:            Stream the kata runtime's console
- open_kata_shell:              Open a shell into the kata runtime
- agent_pull_image:             Run PullImage command against the agent with agent-ctl
- agent_create_container:       Run CreateContainer command against the agent with agent-ctl
- test:                         Test using kata with containerd
- test_capture_logs:            Test using kata with containerd and capture the logs in the user's home directory

Options:
    -d: Enable debug
    -h: Display this help
EOT
    # if script sourced don't exit as this will exit the main shell, just return instead
    [[ $_ != $0 ]] && return "$exit_code" || exit "$exit_code"
}

build_and_install_all() {
    initialize
    build_and_install_kata_runtime
    configure
    create_a_local_rootfs
    build_and_install_rootfs
    install_guest_kernel_image
    build_qemu
    build_bundle_dir_if_necessary
    build_agent_ctl
    check_kata_runtime
    if [ "${KUBERNETES}" == "yes" ]; then
        init_kubernetes
    fi
}

rebuild_and_install_kata() {
    check_out_repos
    build_and_install_kata_runtime
    build_and_add_agent_to_rootfs
    build_and_install_rootfs
    check_kata_runtime
}

# Based on the jenkins_job_build.sh script in kata-containers/tests/.ci - checks out source code and installs dependencies
initialize() {
    # We need git to checkout and bootstrap the ci scripts
    sudo apt-get update && sudo apt-get install -y git socat qemu-utils 
    
    PROFILE="${HOME}/.profile"
    grep -qxF "export GOPATH=\${HOME}/go" "${PROFILE}" || echo "export GOPATH=\${HOME}/go" >> "${PROFILE}"
    grep -qxF "export GOROOT=/usr/local/go" "${PROFILE}" || echo "export GOROOT=/usr/local/go" >> "${PROFILE}"
    grep -qxF "export PATH=\${GOPATH}/bin:/usr/local/go/bin:/usr/sbin:/sbin:\${PATH}" "${PROFILE}" || echo "export PATH=\${GOPATH}/bin:/usr/local/go/bin:/usr/sbin:/sbin:\${PATH}" >> "${PROFILE}"
    . "${HOME}/.profile"
    mkdir -p "${GOPATH}"

    check_out_repos

    pushd "${tests_repo_dir}"
    ci_dir_name=".ci"
    "${ci_dir_name}/install_go.sh" -p -f
    "${ci_dir_name}/install_rust.sh"

    # Run setup, but don't install kata as we will build it ourselves in locations matching the developer guide
    export INSTALL_KATA="no"
    ${ci_dir_name}/setup.sh
    popd
}

check_out_repos() {
    echo "Creating repo: ${tests_repo} and branch ${tests_branch} into ${tests_repo_dir}..."
    mkdir -p $(dirname "${tests_repo_dir}") && sudo chown -R ${USER}:${USER} $(dirname "${tests_repo_dir}")
    [ -d "${tests_repo_dir}" ] || git clone "https://${tests_repo}.git" "${tests_repo_dir}"
    pushd "${tests_repo_dir}"
    git fetch
    if [ -n "${tests_branch}" ]; then
        git checkout ${tests_branch}
    fi
    git reset --hard origin/${tests_branch}
    popd

    echo "Creating repo: ${katacontainers_repo} and branch ${katacontainers_branch} into ${katacontainers_repo_dir}..."
    mkdir -p $(dirname "${katacontainers_repo_dir}") && sudo chown -R ${USER}:${USER} $(dirname "${katacontainers_repo_dir}")
    [ -d "${katacontainers_repo_dir}" ] || git clone "https://${katacontainers_repo}.git" "${katacontainers_repo_dir}"
    pushd "${katacontainers_repo_dir}"
    git fetch
    if [ -n "${katacontainers_branch}" ]; then
        git checkout ${katacontainers_branch}
    fi
    git reset --hard origin/${katacontainers_branch}
    popd
}

build_and_install_kata_runtime() {
    cd ${katacontainers_repo_dir}/src/runtime
    make clean && make && sudo -E PATH=$PATH make install
    debug_output "We should have created Kata runtime binaries:: /usr/local/bin/kata-runtime and /usr/local/bin/containerd-shim-kata-v2"
    debug_output "We should have made the Kata configuration file: /usr/share/defaults/kata-containers/configuration.toml"
    debug_output "kata-runtime version: $(kata-runtime version)"
}

configure() {
    debug_function configure_kata_to_use_rootfs
    debug_function enable_full_debug
    sudo systemctl restart containerd # Ensure containerd picks up debug configuration
}

configure_kata_to_use_rootfs() {
    sudo mkdir -p /etc/kata-containers/
    sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
    sudo sed -i 's/^\(initrd =.*\)/# \1/g' /etc/kata-containers/configuration.toml
}

enable_full_debug() {
    sudo mkdir -p /etc/kata-containers/
    sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
    
    # Note: if all enable_debug are set to true the agent console doesn't seem to work, so only enable the agent and runtime versions
    # TODO LATER - try and work out why this is so we can replace the 2 lines below and stop it being so brittle sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' /etc/kata-containers/configuration.toml
    sudo sed -z -i 's/\(# If enabled, make the agent display debug-level messages.\)\n\(# (default: disabled)\)\n#\(enable_debug = true\)\n/\1\n\2\n\3\n/' /etc/kata-containers/configuration.toml
    sudo sed -z -i 's/\(# system log\)\n\(# (default: disabled)\)\n#\(enable_debug = true\)\n/\1\n\2\n\3\n/' /etc/kata-containers/configuration.toml

    sudo sed -i -e 's/^# *\(debug_console_enabled\).*=.*$/\1 = true/g' /etc/kata-containers/configuration.toml
    sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug initcall_debug"/g' /etc/kata-containers/configuration.toml
}

build_and_add_agent_to_rootfs() {
    debug_function build_a_custom_kata_agent
    debug_function add_custom_agent_to_rootfs
}

build_a_custom_kata_agent() {
    if [[ ! -L /bin/musl-g++ ]]
    then
        rustup target add x86_64-unknown-linux-musl
        sudo ln -s /usr/bin/g++ /bin/musl-g++
    fi
    . "$HOME/.cargo/env"
    cd ${katacontainers_repo_dir}/src/agent && make
    debug_output "Kata agent built: $(ls -al ${katacontainers_repo_dir}/src/agent/target/x86_64-unknown-linux-musl/release/kata-agent)"
    # Run a make install into the rootfs directory in order to create the kata-agent.service file which is required when we add to the rootfs
    sudo -E PATH=$PATH make install DESTDIR="${ROOTFS}"
}

create_a_local_rootfs() {
    sudo rm -rf "${ROOTFS_DIR}"
    cd ${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder
    runc_output=$(sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime")
    echo "Checking that runc is the default docker runtime: ${runc_output}"
    export distro=fedora # I picked fedora as it supports s390x and x86_64 and uses the fedora registry, so we don't get DockerHub toomanyrequests issues
    script -fec 'sudo -E GOPATH=$GOPATH EXTRA_PKGS="vim iputils net-tools iproute skopeo gnupg gpgme-devel" USE_DOCKER=true SECCOMP=yes ./rootfs.sh -r ${ROOTFS_DIR} ${distro}'
    
    # Add umoci binary - TODO LATER replace with rpm when available in fedora
    arch=amd64
    mkdir -p ${ROOTFS_DIR}/usr/local/bin/
    sudo curl -Lo ${ROOTFS_DIR}/usr/local/bin/umoci https://github.com/opencontainers/umoci/releases/download/v0.4.7/umoci.${arch}
    sudo chmod u+x ${ROOTFS_DIR}/usr/local/bin/umoci

    # During the ./rootfs.sh call the kata agent is built as root, so we need to update the permissions, so we can rebuild it
    sudo chown -R ${USER}:${USER} "${katacontainers_repo_dir}/src/agent/"
}

add_custom_agent_to_rootfs() {
    cd ${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder
    sudo install -o root -g root -m 0550 -t ${ROOTFS_DIR}/usr/bin ../../../src/agent/target/x86_64-unknown-linux-musl/release/kata-agent
    sudo install -o root -g root -m 0440 ../../../src/agent/kata-agent.service ${ROOTFS_DIR}/usr/lib/systemd/system/
    sudo install -o root -g root -m 0440 ../../../src/agent/kata-containers.target ${ROOTFS_DIR}/usr/lib/systemd/system/
    debug_output "Added kata agent to rootfs: $(ls -al ${ROOTFS_DIR}/usr/bin/kata-agent)"
}

build_and_install_rootfs() {
    debug_function build_rootfs_image
    debug_function install_rootfs_image
}

build_rootfs_image() {
    cd ${katacontainers_repo_dir}/tools/osbuilder/image-builder
    script -fec 'sudo -E USE_DOCKER=true ./image_builder.sh ${ROOTFS_DIR}'
}

install_rootfs_image() {
    cd ${katacontainers_repo_dir}/tools/osbuilder/image-builder
    commit=$(git log --format=%h -1 HEAD)
    date=$(date +%Y-%m-%d-%T.%N%z)
    image="kata-containers-${date}-${commit}"
    sudo install -o root -g root -m 0640 -D kata-containers.img "/usr/share/kata-containers/${image}"
    (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
    echo "Built Rootfs from ${ROOTFS_DIR} to /usr/share/kata-containers/${image}"
    ls -al /usr/share/kata-containers/
}

install_guest_kernel_image() {
    cd ${katacontainers_repo_dir}/tools/packaging/kernel
    ./build-kernel.sh setup
    ./build-kernel.sh build
    sudo chmod 777 /usr/share/kata-containers/ # Give user permission to install kernel
    ./build-kernel.sh install
    debug_output "New kernel installed to $(ls -al /usr/share/kata-containers/vmlinux*)"
}

build_qemu() {
    ${tests_repo_dir}/.ci/install_qemu.sh
}

check_kata_runtime() {
    sudo kata-runtime check
}

init_kubernetes() {
    # If kubernetes init has previous run we need to clean it by removing the image and resetting k8s
        cid=$(docker ps -a -q -f name=^/kata-registry$)
    if [ -n "${cid}" ]; then
        docker rm ${cid}
    fi
    k8s_nodes=$(kubectl get nodes -o name)
    if [ -n "${k8s_nodes}" ]; then
        kubeadm reset -f
    fi

    export CI="true" && ${tests_repo_dir}/integration/kubernetes/init.sh
    cat << EOT | tee ~/nginx-kata.yaml
apiVersion: v1
kind: Pod
metadata:
  name: nginx-kata
spec:
  runtimeClassName: kata
  containers:
  - name: nginx
    image: nginx
EOT
}

create_kata_pod() {
    kubectl apply -f ~/nginx-kata.yaml
    kubectl get pods
}

delete_kata_pod() {
    kubectl delete -f ~/nginx-kata.yaml
}

restart_kata_pod() {
    delete_kata_pod
    create_kata_pod
}

test_kata_runtime() {
    echo "Running ctr with the kata runtime..."
    test_image="docker.io/library/busybox:latest"
    sudo ctr image pull "${test_image}"
    # If you hit too many requests run `sudo ctr image pull "docker.io/library/busybox:latest" -u <dockerhub username>` command and retry
    sudo ctr run --runtime "io.containerd.kata.v2" --rm -t "${test_image}" test-kata uname -a
}

run_kata_and_capture_logs() {
    echo "Clearing systemd journal..."
    sudo systemctl stop systemd-journald
    sudo rm -f /var/log/journal/*/* /run/log/journal/*/*
    sudo systemctl start systemd-journald
    test_kata_runtime
    echo "Collecting logs..."
    sudo journalctl -q -o cat -a -t kata-runtime > ~/kata-runtime.log
    sudo journalctl -q -o cat -a -t kata > ~/shimv2.log
    echo "Logs output to ~/kata-runtime.log and ~/shimv2.log"
}

get_ids() {
    guest_cid=$(ps -ef | grep qemu-system-x86_64 | egrep -o "guest-cid=[0-9]*" | cut -d= -f2) && sandbox_id=$(ps -ef | grep qemu | egrep -o "sandbox-[^,][^,]*" | sed 's/sandbox-//g' | awk '{print $1}')
}

open_kata_console() {
    get_ids
    sudo -E sandbox_id=${sandbox_id} su -c 'cd /var/run/vc/vm/${sandbox_id} && socat "stdin,raw,echo=0,escape=0x11" "unix-connect:console.sock"'
}

open_kata_shell() {
    get_ids
    sudo kata-runtime exec ${sandbox_id}
}

build_bundle_dir_if_necessary() {
    bundle_dir="/tmp/bundle"
    if [ ! -d "${bundle_dir}" ]; then
        rootfs_dir="$bundle_dir/rootfs"
        image="busybox"
        mkdir -p "$rootfs_dir" && (cd "$bundle_dir" && runc spec)
        sudo docker export $(sudo docker create "$image") | tar -C "$rootfs_dir" -xvf -
    fi
    # There were errors in create container agent-ctl command due to /bin/ seemingly not being on the path, so hardcode it
    sudo sed -i -e 's%^\(\t*\)"sh"$%\1"/bin/sh"%g' "${bundle_dir}/config.json"
}

build_agent_ctl() {
    cd ${GOPATH}/src/${katacontainers_repo}/tools/agent-ctl/
    sudo chown -R ${USER}:${USER} "${HOME}/.cargo/registry"
    make
    cd "./target/x86_64-unknown-linux-musl/release"
}

run_agent_ctl_command() {
    get_ids
    build_bundle_dir_if_necessary
    command=$1
    # If kata-agent-ctl pre-built in this directory, use it directly, otherwise build it first and switch to release
    if [ ! -x kata-agent-ctl ]; then
        build_agent_ctl
    fi 
     ./kata-agent-ctl -l debug connect --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:1024" -c "${command}"
}

agent_pull_image() {
    run_agent_ctl_command "PullImage image=${PULL_IMAGE} cid=${CONTAINER_ID}"
}


agent_create_container() {
    run_agent_ctl_command "CreateContainer cid=${CONTAINER_ID}"
}

main() {
    while getopts "dh" opt; do
        case "$opt" in
            d) 
                DEBUG="-d"
                ;;
            h) 
                usage 0
                ;;
            \?)
                echo "Invalid option: -$OPTARG" >&2
                usage 1
                ;;
        esac
    done

    shift $((OPTIND - 1))

    subcmd="${1:-}"

    [ -z "${subcmd}" ] && usage 1

    case "${subcmd}" in
        all)
            build_and_install_all
            run_kata_and_capture_logs
            ;;
        build_and_install_all)
            build_and_install_all
            ;;
        rebuild_and_install_kata)
            rebuild_and_install_kata
            ;;
        initialize)
            initialize
            ;;
        build_kata_runtime)
            build_and_install_kata_runtime
            ;;
        configure)
            configure
            ;;
        create_rootfs)
            create_a_local_rootfs
            ;;
        build_and_add_agent_to_rootfs)
            build_and_add_agent_to_rootfs
            ;;
        build_and_install_rootfs)
            build_and_install_rootfs
            ;;
        install_guest_kernel)
            install_guest_kernel_image
            ;;
        build_qemu)
            build_qemu
            ;;
        init_kubernetes)
            init_kubernetes
            ;;
        create_kata_pod)
            create_kata_pod
            ;;
        delete_kata_pod)
            delete_kata_pod
            ;;
        restart_kata_pod)
            restart_kata_pod
            ;;
        test)
            test_kata_runtime
            ;;
        test_capture_logs)
            run_kata_and_capture_logs
            ;;
        open_kata_console)
            open_kata_console
            ;;
        open_kata_shell)
            open_kata_shell
            ;;
        agent_pull_image)
            agent_pull_image
            ;;
        agent_create_container)
            agent_create_container
            ;;
        *)
            usage 1
            ;;
    esac
}

main $@