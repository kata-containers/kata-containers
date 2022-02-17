#!/bin/bash -e
#
# Copyright (c) 2021, 2022 IBM Corporation
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
- build_qemu:                       Checkout, patch, build and install QEMU
- configure:                        Configure Kata to use rootfs and enable debug
- connect_to_ssh_demo_pod:          Ssh into the ssh demo pod, showing that the decryption succeeded
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
- open_kata_console:                Stream the kata runtime's console
- open_kata_shell:                  Open a shell into the kata runtime
- rebuild_and_install_kata:         Rebuild the kata runtime and agent and build and install the image
- shim_pull_image:                  Run PullImage command against the shim with ctr
- test_capture_logs:                Test using kata with containerd and capture the logs in the user's home directory
- test:                             Test using kata with containerd

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
    # We need git to checkout and bootstrap the ci scripts and some other packages used in testing
    sudo apt-get update && sudo apt-get install -y curl git socat qemu-utils
    
    grep -qxF "export GOPATH=\${HOME}/go" "${PROFILE}" || echo "export GOPATH=\${HOME}/go" >> "${PROFILE}"
    grep -qxF "export GOROOT=/usr/local/go" "${PROFILE}" || echo "export GOROOT=/usr/local/go" >> "${PROFILE}"
    grep -qxF "export PATH=\${GOPATH}/bin:/usr/local/go/bin:\${PATH}" "${PROFILE}" || echo "export PATH=\${GOPATH}/bin:/usr/local/go/bin:\${PATH}" >> "${PROFILE}"
    
    # Load the new go and PATH parameters from the profile
    . ${PROFILE}
    mkdir -p "${GOPATH}"

    check_out_repos

    pushd "${tests_repo_dir}"
    local ci_dir_name=".ci"
    sudo -E PATH=$PATH -s "${ci_dir_name}/install_go.sh" -p -f
    sudo -E PATH=$PATH -s "${ci_dir_name}/install_rust.sh"

    # Run setup, but don't install kata as we will build it ourselves in locations matching the developer guide
    export INSTALL_KATA="no"
    sudo -E PATH=$PATH -s ${ci_dir_name}/setup.sh
    # Reload the profile to pick up installed dependencies
    . ${PROFILE}
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
    pushd ${katacontainers_repo_dir}/src/runtime
    make clean && make && sudo -E PATH=$PATH make install
    debug_output "We should have created Kata runtime binaries:: /usr/local/bin/kata-runtime and /usr/local/bin/containerd-shim-kata-v2"
    debug_output "We should have made the Kata configuration file: /usr/share/defaults/kata-containers/configuration.toml"
    debug_output "kata-runtime version: $(kata-runtime version)"
    popd
}

configure() {
    debug_function configure_kata_to_use_rootfs
    debug_function enable_full_debug
    # Temp PoC verify code: Inject policy path config parameter
    sudo sed -i -e 's%^kernel_params = "\(.*\)"%kernel_params = "\1 agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"%g' /etc/kata-containers/configuration.toml

    # If using AA then need to add the agent_config
    if [ "${AA_KBC}" == "offline_fs_kbc" ]; then
        sudo sed -i -e 's%^kernel_params = "\(.*\)"%kernel_params = "\1 agent.config_file=/etc/agent-config.toml"%g' /etc/kata-containers/configuration.toml
    fi

    # insert the cri_handler = "cc" into the [plugins.cri.containerd.runtimes.kata] section
    sudo sed -z -i 's/\([[:blank:]]*\)\(runtime_type = "io.containerd.kata.v2"\)/\1\2\n\1cri_handler = "cc"/' /etc/containerd/config.toml

    # Add cni directory to containerd config
    echo "    [plugins.cri.cni]
      # conf_dir is the directory in which the admin places a CNI conf.
      conf_dir = \"/etc/cni/net.d\"" | sudo tee -a /etc/containerd/config.toml
    
    # Switch image offload to true in kata config
    sudo sed -i -e 's/^# *\(service_offload\).*=.*$/\1 = true/g' /etc/kata-containers/configuration.toml

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
    . "$HOME/.cargo/env"
    pushd ${katacontainers_repo_dir}/src/agent
    sudo -E PATH=$PATH make

    ARCH=$(uname -m)
    [ ${ARCH} == "ppc64le" ] || [ ${ARCH} == "s390x" ] && export LIBC=gnu || export LIBC=musl
    [ ${ARCH} == "ppc64le" ] && export ARCH=powerpc64le

    debug_output "Kata agent built: $(ls -al ${katacontainers_repo_dir}/src/agent/target/${ARCH}-unknown-linux-${LIBC}/release/kata-agent)"
    # Run a make install into the rootfs directory in order to create the kata-agent.service file which is required when we add to the rootfs
    sudo -E PATH=$PATH make install DESTDIR="${ROOTFS}"
    popd
}

create_a_local_rootfs() {
    sudo rm -rf "${ROOTFS_DIR}"
    pushd ${katacontainers_repo_dir}/tools/osbuilder/rootfs-builder
    export distro="ubuntu"
    [[ -z "${USE_PODMAN:-}" ]] && use_docker="${use_docker:-1}"
    sudo -E OS_VERSION="${OS_VERSION:-}" GOPATH=$GOPATH EXTRA_PKGS="vim iputils-ping net-tools" DEBUG="${DEBUG}" USE_DOCKER="${use_docker:-}" SKOPEO=${SKOPEO:-} AA_KBC=${AA_KBC:-} UMOCI=yes SECCOMP=yes ./rootfs.sh -r ${ROOTFS_DIR} ${distro}

     # Install_rust.sh during rootfs.sh switches us to the main branch of the tests repo, so switch back now
    pushd "${tests_repo_dir}"
    git checkout ${tests_branch}
    popd
    # During the ./rootfs.sh call the kata agent is built as root, so we need to update the permissions, so we can rebuild it
    sudo chown -R ${USER}:${USER} "${katacontainers_repo_dir}/src/agent/"

    # If offline key broker set then include ssh-demo keys and config from
    # https://github.com/confidential-containers/documentation/tree/main/demos/ssh-demo
    if [ "${AA_KBC}" == "offline_fs_kbc" ]; then
        curl -Lo "${HOME}/aa-offline_fs_kbc-keys.json" https://raw.githubusercontent.com/confidential-containers/documentation/main/demos/ssh-demo/aa-offline_fs_kbc-keys.json
        sudo mv "${HOME}/aa-offline_fs_kbc-keys.json" "${ROOTFS_DIR}/etc/aa-offline_fs_kbc-keys.json"
        local rootfs_agent_config="${ROOTFS_DIR}/etc/agent-config.toml"
        sudo -E AA_KBC_PARAMS="offline_fs_kbc::null" envsubst < ${katacontainers_repo_dir}/docs/how-to/data/confidential-agent-config.toml.in | sudo tee ${rootfs_agent_config}
    fi

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
    debug_output "Added kata agent to rootfs: $(ls -al ${ROOTFS_DIR}/usr/bin/kata-agent)"
    popd
}

build_and_install_rootfs() {
    debug_function build_rootfs_image
    debug_function install_rootfs_image
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
    sudo install -o root -g root -m 0640 -D kata-containers.img "/usr/share/kata-containers/${image}"
    (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
    echo "Built Rootfs from ${ROOTFS_DIR} to /usr/share/kata-containers/${image}"
    ls -al /usr/share/kata-containers/
    popd
}

install_guest_kernel_image() {
    pushd ${katacontainers_repo_dir}/tools/packaging/kernel
    sudo -E PATH=$PATH ./build-kernel.sh setup
    sudo -E PATH=$PATH ./build-kernel.sh build
    sudo chmod u+wrx /usr/share/kata-containers/ # Give user permission to install kernel
    sudo -E PATH=$PATH ./build-kernel.sh install
    debug_output "New kernel installed to $(ls -al /usr/share/kata-containers/vmlinux*)"
    popd
}

build_qemu() {
    ${tests_repo_dir}/.ci/install_qemu.sh
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

kubernetes_create_cc_pod() {
    kubectl apply -f ${k8s_pod_file} && pod=$(kubectl get pods -o jsonpath='{.items..metadata.name}') && kubectl wait --for=condition=ready pods/$pod
    kubectl get pod $pod
}

kubernetes_delete_cc_pod() {
    kubectl delete -f ${k8s_pod_file}
}

# Check out the doc repo if required and pushd
pushd_ssh_demo() {
    local doc_repo=github.com/confidential-containers/documentation
    local doc_repo_dir="${GOPATH}/src/${doc_repo}"
    mkdir -p $(dirname ${doc_repo_dir}) && sudo chown -R ${USER}:${USER} $(dirname ${doc_repo_dir})
    if [ ! -d "${doc_repo_dir}" ]; then
        git clone https://${doc_repo} "${doc_repo_dir}"
        pushd "${doc_repo_dir}/demos/ssh-demo"
        # Update runtimeClassName from kata-cc to kata
        sudo sed -i -e 's/\([[:blank:]]*runtimeClassName: \).*/\1kata/g' "${doc_repo_dir}/demos/ssh-demo/k8s-cc-ssh.yaml"
        chmod 600 ccv0-ssh
    else 
        pushd "${doc_repo_dir}/demos/ssh-demo"
    fi
}

kubernetes_create_ssh_demo_pod() {
    pushd_ssh_demo
    kubectl apply -f k8s-cc-ssh.yaml && pod=$(kubectl get pods -o jsonpath='{.items..metadata.name}') && kubectl wait --for=condition=ready pods/$pod
    kubectl get pod $pod
    popd
}

connect_to_ssh_demo_pod() {
    local doc_repo=github.com/confidential-containers/documentation
    local doc_repo_dir="${GOPATH}/src/${doc_repo}"
    local ssh_command="ssh -i ${doc_repo_dir}/demos/ssh-demo/ccv0-ssh root@$(kubectl get service ccv0-ssh -o jsonpath="{.spec.clusterIP}")"
    echo "Issuing command '${ssh_command}'"
    ${ssh_command}
}

kubernetes_delete_ssh_demo_pod() {
    pushd_ssh_demo
    kubectl delete -f k8s-cc-ssh.yaml
    popd
}

crictl_sandbox_name=kata-cc-busybox-sandbox
crictl_create_cc_pod() {
    # Update iptables to allow forwarding to the cni0 bridge avoiding issues caused by the docker0 bridge
    sudo iptables -P FORWARD ACCEPT
    
    # Create crictl pod config
cat << EOF > ${HOME}/pod-config.yaml
metadata:
  name: ${crictl_sandbox_name}
EOF

    # If already exists then delete and re-create
    if [ -n "$(sudo crictl pods --name ${crictl_sandbox_name} -q)" ]; then
        crictl_delete_cc
    fi

    $(sudo crictl runp -r kata ${HOME}/pod-config.yaml)
    sudo crictl pods
}

crictl_create_cc_container() {
    # Create container configuration yaml based on our test copy of busybox
    cat << EOF > ${HOME}/container-config.yaml
metadata:
  name: kata-cc-busybox
image:
  image: quay.io/kata-containers/confidential-containers:signed
command:
- top
log_path: kata-cc.0.log
EOF

    local pod_id=$(sudo crictl pods --name ${crictl_sandbox_name} -q)
    local container_id=$(sudo crictl create -with-pull ${pod_id} ${HOME}/container-config.yaml ${HOME}/pod-config.yaml)
    sudo crictl start ${container_id}
    sudo crictl ps -a
}

crictl_delete_cc() {
    local pod_id=$(sudo crictl pods --name ${crictl_sandbox_name} -q)
    local container_id=$(sudo crictl ps --pod ${pod_id} -q)
    if [ -n "${container_id}" ]; then
        sudo crictl stop ${container_id} && sudo crictl rm ${container_id}
    fi
    sudo crictl stopp ${pod_id} && sudo crictl rmp ${pod_id}
}

test_kata_runtime() {
    echo "Running ctr with the kata runtime..."
    local test_image="quay.io/kata-containers/confidential-containers:signed"
    if [ -z $(ctr images ls -q name=="${test_image}") ]; then
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
    sandbox_id=$(ps -ef | grep qemu | egrep -o "sandbox-[^,][^,]*" | sed 's/sandbox-//g' | awk '{print $1}')
}

open_kata_console() {
    get_ids
    sudo -E sandbox_id=${sandbox_id} su -c 'cd /var/run/vc/vm/${sandbox_id} && socat "stdin,raw,echo=0,escape=0x11" "unix-connect:console.sock"'
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

main() {
    while getopts "dh" opt; do
        case "$opt" in
            d) 
                DEBUG="-d"
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
        build_qemu)
            build_qemu
            ;;
        init_kubernetes)
            init_kubernetes
            ;;
        crictl_create_cc_pod)
            crictl_create_cc_pod
            ;;
        crictl_create_cc_container)
            crictl_create_cc_container
            ;;
        crictl_delete_cc)
            crictl_delete_cc
            ;;
        kubernetes_create_cc_pod)
            kubernetes_create_cc_pod
            ;;
        kubernetes_delete_cc_pod)
            kubernetes_delete_cc_pod
            ;;
        kubernetes_create_ssh_demo_pod)
            kubernetes_create_ssh_demo_pod
            ;;
        connect_to_ssh_demo_pod)
            connect_to_ssh_demo_pod
            ;;
        kubernetes_delete_ssh_demo_pod)
            kubernetes_delete_ssh_demo_pod
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
        *)
            usage 1
            ;;
    esac
}

main $@
