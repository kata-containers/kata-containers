#!/bin/bash -e
#
# Copyright (c) 2021, 2023 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Disclaimer: This script is work in progress for supporting the CCv0 prototype
# It shouldn't be considered supported by the Kata Containers community, or anyone else

# Based on https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md,
# but with elements of the tests/.ci scripts used

readonly script_name="$(basename "${BASH_SOURCE[0]}")"

# By default in Golang >= 1.16 GO111MODULE is set to "on", but not all modules support it, so overwrite to "auto"
export GO111MODULE="auto"

# Setup kata containers environments if not set - we default to use containerd
export CRI_CONTAINERD=${CRI_CONTAINERD:-"yes"}
export CRI_RUNTIME=${CRI_RUNTIME:-"containerd"}
export CRIO=${CRIO:-"no"}
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export KUBERNETES=${KUBERNETES:-"no"}
export AGENT_INIT="${AGENT_INIT:-${TEST_INITRD:-no}}"
export AA_KBC="${AA_KBC:-offline_fs_kbc}"
export KATA_BUILD_CC=${KATA_BUILD_CC:-"yes"}
export TEE_TYPE=${TEE_TYPE:-}
export PREFIX="${PREFIX:-/opt/confidential-containers}"
export RUNTIME_CONFIG_PATH="${RUNTIME_CONFIG_PATH:-${PREFIX}/share/defaults/kata-containers/configuration.toml}"

# Allow the user to overwrite the default repo and branch names if they want to build from a fork
export katacontainers_repo="${katacontainers_repo:-github.com/kata-containers/kata-containers}"
export katacontainers_branch="${katacontainers_branch:-CCv0}" 
export kata_default_branch=${katacontainers_branch}
export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_branch="${tests_branch:-CCv0}"
export target_branch=${tests_branch} # kata-containers/ci/lib.sh uses target branch var to check out tests repo

# if .bash_profile exists then use it, otherwise fall back to .profile
export PROFILE="${HOME}/.profile"
if [ -r "${HOME}/.bash_profile" ]; then
    export PROFILE="${HOME}/.bash_profile"
fi
# Stop PS1: unbound variable error happening
export PS1=${PS1:-}

# Create a bunch of common, derived values up front so we don't need to create them in all the different functions
. ${PROFILE}
if [ -z ${GOPATH} ]; then
    export GOPATH=${HOME}/go
fi
export tests_repo_dir="${GOPATH}/src/${tests_repo}"
export katacontainers_repo_dir="${GOPATH}/src/${katacontainers_repo}"
export ROOTFS_DIR="${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder/rootfs"
export PULL_IMAGE="${PULL_IMAGE:-quay.io/kata-containers/confidential-containers:signed}" # Doesn't need authentication
export CONTAINER_ID="${CONTAINER_ID:-0123456789}"
source /etc/os-release || source /usr/lib/os-release
grep -Eq "\<fedora\>" /etc/os-release 2> /dev/null && export USE_PODMAN=true


# If we've already checked out the test repo then source the confidential scripts
if [ "${KUBERNETES}" == "yes" ]; then
    export BATS_TEST_DIRNAME="${tests_repo_dir}/integration/kubernetes/confidential"
    [ -d "${BATS_TEST_DIRNAME}" ] && source "${BATS_TEST_DIRNAME}/lib.sh"
else
    export BATS_TEST_DIRNAME="${tests_repo_dir}/integration/containerd/confidential"
    [ -d "${BATS_TEST_DIRNAME}" ] && source "${BATS_TEST_DIRNAME}/lib.sh"
fi

[ -d "${BATS_TEST_DIRNAME}" ] && source "${BATS_TEST_DIRNAME}/../../confidential/lib.sh"

usage() {
    exit_code="$1"
    cat <<EOF
Overview:
    Build and test kata containers from source
    Optionally set kata-containers and tests repo and branch as exported variables before running
    e.g. export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=kata-ci-from-fork && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=kata-ci-from-fork && ~/${script_name} build_and_install_all
Usage:
    ${script_name} [options] <command>
Commands:
- agent_create_container:           Run CreateContainer command against the agent with agent-ctl
- agent_pull_image:                 Run PullImage command against the agent with agent-ctl
- all:                              Build and install everything, test kata with containerd and capture the logs
- build_and_add_agent_to_rootfs:    Builds the kata-agent and adds it to the rootfs
- build_and_install_all:            Build and install everything
- build_and_install_rootfs:         Builds and installs the rootfs image
- build_kata_runtime:               Build and install the kata runtime
- build_cloud_hypervisor            Checkout, patch, build and install Cloud Hypervisor
- build_qemu:                       Checkout, patch, build and install QEMU
- configure:                        Configure Kata to use rootfs and enable debug
- connect_to_ssh_demo_pod:          Ssh into the ssh demo pod, showing that the decryption succeeded
- copy_signature_files_to_guest     Copies signature verification files to guest
- create_rootfs:                    Create a local rootfs
- crictl_create_cc_container        Use crictl to create a new busybox container in the kata cc pod
- crictl_create_cc_pod              Use crictl to create a new kata cc pod
- crictl_delete_cc                  Use crictl to delete the kata cc pod sandbox and container in it
- help:                             Display this help
- init_kubernetes:                  initialize a Kubernetes cluster on this system
- initialize:                       Install dependencies and check out kata-containers source
- install_guest_kernel:             Setup, build and install the guest kernel
- kubernetes_create_cc_pod:         Create a Kata CC runtime busybox-based pod in Kubernetes
- kubernetes_create_ssh_demo_pod:   Create a Kata CC runtime pod based on the ssh demo
- kubernetes_delete_cc_pod:         Delete the Kata CC runtime busybox-based pod in Kubernetes
- kubernetes_delete_ssh_demo_pod:   Delete the Kata CC runtime pod based on the ssh demo
- open_kata_shell:                  Open a shell into the kata runtime
- rebuild_and_install_kata:         Rebuild the kata runtime and agent and build and install the image
- shim_pull_image:                  Run PullImage command against the shim with ctr
- test_capture_logs:                Test using kata with containerd and capture the logs in the user's home directory
- test:                             Test using kata with containerd

Options:
    -d: Enable debug
    -h: Display this help
EOF
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
    case "$KATA_HYPERVISOR" in
        "qemu") 
            build_qemu
            ;;
        "cloud-hypervisor") 
            build_cloud_hypervisor
            ;;
        *)
            echo "Invalid option: $KATA_HYPERVISOR is not supported." >&2
            ;;
    esac

    check_kata_runtime
    if [ "${KUBERNETES}" == "yes" ]; then
        init_kubernetes
    fi
}

rebuild_and_install_kata() {
    checkout_tests_repo
    checkout_kata_containers_repo
    build_and_install_kata_runtime
    build_and_add_agent_to_rootfs
    build_and_install_rootfs
    check_kata_runtime
}

# Based on the jenkins_job_build.sh script in kata-containers/tests/.ci - checks out source code and installs dependencies
initialize() {
    # We need git to checkout and bootstrap the ci scripts and some other packages used in testing
    sudo apt-get update && sudo apt-get install -y curl git qemu-utils
    
    grep -qxF "export GOPATH=\${HOME}/go" "${PROFILE}" || echo "export GOPATH=\${HOME}/go" >> "${PROFILE}"
    grep -qxF "export GOROOT=/usr/local/go" "${PROFILE}" || echo "export GOROOT=/usr/local/go" >> "${PROFILE}"
    grep -qxF "export PATH=\${GOPATH}/bin:/usr/local/go/bin:\${PATH}" "${PROFILE}" || echo "export PATH=\${GOPATH}/bin:/usr/local/go/bin:\${PATH}" >> "${PROFILE}"
    
    # Load the new go and PATH parameters from the profile
    . ${PROFILE}
    mkdir -p "${GOPATH}"

    checkout_tests_repo

    pushd "${tests_repo_dir}"
    local ci_dir_name=".ci"
    sudo -E PATH=$PATH -s "${ci_dir_name}/install_go.sh" -p -f
    sudo -E PATH=$PATH -s "${ci_dir_name}/install_rust.sh"
    # Need to change ownership of rustup so later process can create temp files there
    sudo chown -R ${USER}:${USER} "${HOME}/.rustup"

    checkout_kata_containers_repo

    # Run setup, but don't install kata as we will build it ourselves in locations matching the developer guide
    export INSTALL_KATA="no"
    sudo -E PATH=$PATH -s ${ci_dir_name}/setup.sh
    # Reload the profile to pick up installed dependencies
    . ${PROFILE}
    popd
}

checkout_tests_repo() {
    echo "Creating repo: ${tests_repo} and branch ${tests_branch} into ${tests_repo_dir}..."
    # Due to git https://github.blog/2022-04-12-git-security-vulnerability-announced/ the tests repo needs
    # to be owned by root as it is re-checked out in rootfs.sh
    mkdir -p $(dirname "${tests_repo_dir}")
    [ -d "${tests_repo_dir}" ] || sudo -E git clone "https://${tests_repo}.git" "${tests_repo_dir}"
    sudo -E chown -R root:root "${tests_repo_dir}"
    pushd "${tests_repo_dir}"
    sudo -E git fetch
    if [ -n "${tests_branch}" ]; then
        sudo -E git checkout ${tests_branch}
    fi
    sudo -E git reset --hard origin/${tests_branch}
    popd

    source "${BATS_TEST_DIRNAME}/lib.sh"
    source "${BATS_TEST_DIRNAME}/../../confidential/lib.sh"
}

# Note: clone_katacontainers_repo using go, so that needs to be installed first
checkout_kata_containers_repo() {
    source "${tests_repo_dir}/.ci/lib.sh"
    echo "Creating repo: ${katacontainers_repo} and branch ${kata_default_branch} into ${katacontainers_repo_dir}..."
    clone_katacontainers_repo
    sudo -E chown -R ${USER}:${USER} "${katacontainers_repo_dir}"
}

build_and_install_kata_runtime() {
    export DEFAULT_HYPERVISOR=${KATA_HYPERVISOR}
    ${tests_repo_dir}/.ci/install_runtime.sh
}

configure() {
    # configure kata to use rootfs, not initrd
    sudo sed -i 's/^\(initrd =.*\)/# \1/g' ${RUNTIME_CONFIG_PATH}

    enable_full_debug
    enable_agent_console

    # Switch image offload to true in kata config
    switch_image_service_offload "on"

    configure_cc_containerd
    # From crictl v1.24.1 the default timoout leads to the pod creation failing, so update it
    sudo crictl config --set timeout=10

    # Verity checks aren't working locally, as we aren't re-genning the hash maybe? so remove it from the kernel parameters
    remove_kernel_param "cc_rootfs_verity.scheme"
}

build_and_add_agent_to_rootfs() {
    build_a_custom_kata_agent
    add_custom_agent_to_rootfs
}

build_a_custom_kata_agent() {
    # Install libseccomp for static linking
    sudo -E PATH=$PATH GOPATH=$GOPATH ${katacontainers_repo_dir}/ci/install_libseccomp.sh /tmp/kata-libseccomp /tmp/kata-gperf
    export LIBSECCOMP_LINK_TYPE=static
    export LIBSECCOMP_LIB_PATH=/tmp/kata-libseccomp/lib

    . "$HOME/.cargo/env"
    pushd ${katacontainers_repo_dir}/src/agent
    sudo -E PATH=$PATH make

    ARCH=$(uname -m)
    [ ${ARCH} == "ppc64le" ] || [ ${ARCH} == "s390x" ] && export LIBC=gnu || export LIBC=musl
    [ ${ARCH} == "ppc64le" ] && export ARCH=powerpc64le

    # Run a make install into the rootfs directory in order to create the kata-agent.service file which is required when we add to the rootfs
    sudo -E PATH=$PATH make install DESTDIR="${ROOTFS_DIR}"
    popd
}

create_a_local_rootfs() {
    sudo rm -rf "${ROOTFS_DIR}"
    pushd ${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder
    export distro="ubuntu"
    [[ -z "${USE_PODMAN:-}" ]] && use_docker="${use_docker:-1}"
    sudo -E OS_VERSION="${OS_VERSION:-}" GOPATH=$GOPATH EXTRA_PKGS="vim iputils-ping net-tools" DEBUG="${DEBUG:-}" USE_DOCKER="${use_docker:-}" SKOPEO=${SKOPEO:-} AA_KBC=${AA_KBC:-} UMOCI=yes SECCOMP=yes ./rootfs.sh -r ${ROOTFS_DIR} ${distro}

     # Install_rust.sh during rootfs.sh switches us to the main branch of the tests repo, so switch back now
    pushd "${tests_repo_dir}"
    sudo -E git checkout ${tests_branch}
    popd
    # During the ./rootfs.sh call the kata agent is built as root, so we need to update the permissions, so we can rebuild it
    sudo chown -R ${USER}:${USER} "${katacontainers_repo_dir}/src/agent/"

    popd
}

add_custom_agent_to_rootfs() {
    pushd ${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder

    ARCH=$(uname -m)
    [ ${ARCH} == "ppc64le" ] || [ ${ARCH} == "s390x" ] && export LIBC=gnu || export LIBC=musl
    [ ${ARCH} == "ppc64le" ] && export ARCH=powerpc64le

    sudo install -o root -g root -m 0550 -t ${ROOTFS_DIR}/usr/bin ${katacontainers_repo_dir}/src/agent/target/${ARCH}-unknown-linux-${LIBC}/release/kata-agent
    sudo install -o root -g root -m 0440 ../../../src/agent/kata-agent.service ${ROOTFS_DIR}/usr/lib/systemd/system/
    sudo install -o root -g root -m 0440 ../../../src/agent/kata-containers.target ${ROOTFS_DIR}/usr/lib/systemd/system/
    popd
}

build_and_install_rootfs() {
    build_rootfs_image
    install_rootfs_image
}

build_rootfs_image() {
    pushd ${katacontainers_repo_dir}/tools/osbuilder/image-builder
    # Logic from install_kata_image.sh - if we aren't using podman (ie on a fedora like), then use docker
    [[ -z "${USE_PODMAN:-}" ]] && use_docker="${use_docker:-1}"
    sudo -E USE_DOCKER="${use_docker:-}" ./image_builder.sh ${ROOTFS_DIR}
    popd
}

install_rootfs_image() {
    pushd ${katacontainers_repo_dir}/tools/osbuilder/image-builder
    local commit=$(git log --format=%h -1 HEAD)
    local date=$(date +%Y-%m-%d-%T.%N%z)
    local image="kata-containers-${date}-${commit}"
    sudo install -o root -g root -m 0640 -D kata-containers.img "${PREFIX}/share/kata-containers/${image}"
    (cd ${PREFIX}/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
    echo "Built Rootfs from ${ROOTFS_DIR} to ${PREFIX}/share/kata-containers/${image}"
    ls -al ${PREFIX}/share/kata-containers
    popd
}

install_guest_kernel_image() {
   ${tests_repo_dir}/.ci/install_kata_kernel.sh
}

build_qemu() {
    ${tests_repo_dir}/.ci/install_virtiofsd.sh
    ${tests_repo_dir}/.ci/install_qemu.sh
}

build_cloud_hypervisor() {
    ${tests_repo_dir}/.ci/install_virtiofsd.sh
    ${tests_repo_dir}/.ci/install_cloud_hypervisor.sh
}

check_kata_runtime() {
    sudo kata-runtime check
}

k8s_pod_file="${HOME}/busybox-cc.yaml"
init_kubernetes() {
    # Check that kubeadm was installed and install it otherwise
    if ! [ -x "$(command -v kubeadm)" ]; then
        pushd "${tests_repo_dir}/.ci"
        sudo -E PATH=$PATH -s install_kubernetes.sh
        if [ "${CRI_CONTAINERD}" == "yes" ]; then
            sudo -E PATH=$PATH -s "configure_containerd_for_kubernetes.sh"
        fi
        popd
    fi

    # If kubernetes init has previously run we need to clean it by removing the image and resetting k8s
    local cid=$(sudo docker ps -a -q -f name=^/kata-registry$)
    if [ -n "${cid}" ]; then
        sudo docker stop ${cid} && sudo docker rm ${cid}
    fi
    local k8s_nodes=$(kubectl get nodes -o name 2>/dev/null || true)
    if [ -n "${k8s_nodes}" ]; then
        sudo kubeadm reset -f
    fi

    export CI="true" && sudo -E PATH=$PATH -s ${tests_repo_dir}/integration/kubernetes/init.sh
    sudo chown ${USER}:$(id -g -n ${USER}) "$HOME/.kube/config"
    cat << EOF > ${k8s_pod_file}
apiVersion: v1
kind: Pod
metadata:
  name: busybox-cc
spec:
  runtimeClassName: kata
  containers:
  - name: nginx
    image: quay.io/kata-containers/confidential-containers:signed
    imagePullPolicy: Always  
EOF
}

call_kubernetes_create_cc_pod() {
    kubernetes_create_cc_pod ${k8s_pod_file}
}

call_kubernetes_delete_cc_pod() {
    pod_name=$(kubectl get pods -o jsonpath='{.items..metadata.name}')
    kubernetes_delete_cc_pod $pod_name
}

call_kubernetes_create_ssh_demo_pod() {
    setup_decryption_files_in_guest
    kubernetes_create_ssh_demo_pod
}

call_connect_to_ssh_demo_pod() {
    connect_to_ssh_demo_pod
}

call_kubernetes_delete_ssh_demo_pod() {
    pod=$(kubectl get pods -o jsonpath='{.items..metadata.name}')
    kubernetes_delete_ssh_demo_pod $pod
}

crictl_sandbox_name=kata-cc-busybox-sandbox
call_crictl_create_cc_pod() {
    # Update iptables to allow forwarding to the cni0 bridge avoiding issues caused by the docker0 bridge
    sudo iptables -P FORWARD ACCEPT
    
    # get_pod_config in tests_common exports `pod_config` that points to the prepared pod config yaml 
    get_pod_config

    crictl_delete_cc_pod_if_exists "${crictl_sandbox_name}"
    crictl_create_cc_pod "${pod_config}"
    sudo crictl pods
}

call_crictl_create_cc_container() {
    # Create container configuration yaml based on our test copy of busybox
    # get_pod_config in tests_common exports `pod_config` that points to the prepared pod config yaml 
    get_pod_config

    local container_config="${FIXTURES_DIR}/${CONTAINER_CONFIG_FILE:-container-config.yaml}"
    local pod_name=${crictl_sandbox_name}
    crictl_create_cc_container ${pod_name} ${pod_config} ${container_config}
    sudo crictl ps -a
}

crictl_delete_cc() {
    crictl_delete_cc_pod ${crictl_sandbox_name}
}

test_kata_runtime() {
    echo "Running ctr with the kata runtime..."
    local test_image="quay.io/kata-containers/confidential-containers:signed"
    if [ -z $(sudo ctr images ls -q name=="${test_image}") ]; then
        sudo ctr image pull "${test_image}"
    fi
    sudo ctr run --runtime "io.containerd.kata.v2" --rm -t "${test_image}" test-kata uname -a
}

run_kata_and_capture_logs() {
    echo "Clearing systemd journal..."
    sudo systemctl stop systemd-journald
    sudo rm -f /var/log/journal/*/* /run/log/journal/*/*
    sudo systemctl start systemd-journald
    test_kata_runtime
    echo "Collecting logs..."
    sudo journalctl -q -o cat -a -t kata-runtime > ${HOME}/kata-runtime.log
    sudo journalctl -q -o cat -a -t kata > ${HOME}/shimv2.log
    echo "Logs output to ${HOME}/kata-runtime.log and ${HOME}/shimv2.log"
}

get_ids() {
    guest_cid=$(sudo ss -H --vsock | awk '{print $6}' | cut -d: -f1)
    sandbox_id=$(ps -ef | grep containerd-shim-kata-v2 | egrep -o "id [^,][^,].* " | awk '{print $2}')
}

open_kata_shell() {
    get_ids
    sudo -E "PATH=$PATH" kata-runtime exec ${sandbox_id}
}

build_bundle_dir_if_necessary() {
    bundle_dir="/tmp/bundle"
    if [ ! -d "${bundle_dir}" ]; then
        rootfs_dir="$bundle_dir/rootfs"
        image="quay.io/kata-containers/confidential-containers:signed"
        mkdir -p "$rootfs_dir" && (cd "$bundle_dir" && runc spec)
        sudo docker export $(sudo docker create "$image") | tar -C "$rootfs_dir" -xvf -
    fi
    # There were errors in create container agent-ctl command due to /bin/ seemingly not being on the path, so hardcode it
    sudo sed -i -e 's%^\(\t*\)"sh"$%\1"/bin/sh"%g' "${bundle_dir}/config.json"
}

build_agent_ctl() {
    cd ${GOPATH}/src/${katacontainers_repo}/src/tools/agent-ctl/
    if [ -e "${HOME}/.cargo/registry" ]; then
        sudo chown -R ${USER}:${USER} "${HOME}/.cargo/registry"
    fi
    sudo -E PATH=$PATH -s  make
    ARCH=$(uname -m)
    [ ${ARCH} == "ppc64le" ] || [ ${ARCH} == "s390x" ] && export LIBC=gnu || export LIBC=musl
    [ ${ARCH} == "ppc64le" ] && export ARCH=powerpc64le
    cd "./target/${ARCH}-unknown-linux-${LIBC}/release/"
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
    run_agent_ctl_command "PullImage image=${PULL_IMAGE} cid=${CONTAINER_ID} source_creds=${SOURCE_CREDS}"
}

agent_create_container() {
    run_agent_ctl_command "CreateContainer cid=${CONTAINER_ID}"
}

shim_pull_image() {
    get_ids
    local ctr_shim_command="sudo ctr --namespace k8s.io shim --id ${sandbox_id} pull-image ${PULL_IMAGE} ${CONTAINER_ID}"
    echo "Issuing command '${ctr_shim_command}'"
    ${ctr_shim_command}
}

call_copy_signature_files_to_guest() {
    # TODO #5173 - remove this once the kernel_params aren't ignored by the agent config
    export DEBUG_CONSOLE="true"
    
    if [ "${SKOPEO:-}" = "yes" ]; then
        add_kernel_params "agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"
        setup_skopeo_signature_files_in_guest
    else
        # TODO #4888 - set config to specifically enable signature verification to be on in ImageClient
        setup_offline_fs_kbc_signature_files_in_guest
    fi
}

main() {
    while getopts "dh" opt; do
        case "$opt" in
            d) 
                export DEBUG="-d"
                set -x
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
        build_cloud_hypervisor)
            build_cloud_hypervisor
            ;;
        build_qemu)
            build_qemu
            ;;
        init_kubernetes)
            init_kubernetes
            ;;
        crictl_create_cc_pod)
            call_crictl_create_cc_pod
            ;;
        crictl_create_cc_container)
            call_crictl_create_cc_container
            ;;
        crictl_delete_cc)
            crictl_delete_cc
            ;;
        kubernetes_create_cc_pod)
            call_kubernetes_create_cc_pod
            ;;
        kubernetes_delete_cc_pod)
            call_kubernetes_delete_cc_pod
            ;;
        kubernetes_create_ssh_demo_pod)
            call_kubernetes_create_ssh_demo_pod
            ;;
        connect_to_ssh_demo_pod)
            call_connect_to_ssh_demo_pod
            ;;
        kubernetes_delete_ssh_demo_pod)
            call_kubernetes_delete_ssh_demo_pod
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
        shim_pull_image)
            shim_pull_image
            ;;
        agent_create_container)
            agent_create_container
            ;;
        copy_signature_files_to_guest)
            call_copy_signature_files_to_guest
            ;;
        *)
            usage 1
            ;;
    esac
}

main $@
