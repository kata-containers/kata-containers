#!/usr/bin/env bash
#
# Copyright (c) 2018-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export repo_root_dir="$(cd "${this_script_dir}/../" && pwd)"

source "${this_script_dir}/error.sh"

# Kata tests directory used for storing various test-related artifacts.
KATA_TESTS_BASEDIR="${KATA_TESTS_BASEDIR:-/var/log/kata-tests}"

# Directory that can be used for storing test logs.
KATA_TESTS_LOGDIR="${KATA_TESTS_LOGDIR:-${KATA_TESTS_BASEDIR}/logs}"

# Directory that can be used for storing test data.
KATA_TESTS_DATADIR="${KATA_TESTS_DATADIR:-${KATA_TESTS_BASEDIR}/data}"

# Directory that can be used for storing cache kata components
KATA_TESTS_CACHEDIR="${KATA_TESTS_CACHEDIR:-${KATA_TESTS_BASEDIR}/cache}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

RUNTIME="${RUNTIME:-containerd-shim-kata-v2}"

export branch="${target_branch:-main}"

function die() {
	local msg="$*"

	if [ -z "${KATA_TEST_VERBOSE:-}" ]; then
		echo -e "[$(basename $0):${BASH_LINENO[0]}] ERROR: $msg" >&2
		exit 1
	fi

	echo >&2 "ERROR: $msg"

	# This function is called to indicate a fatal error occurred, so
	# the caller of this function is the site of the detected error.
	local error_location
	error_location=$(caller 0)

	local line
	local func
	local file

	line=$(echo "$error_location"|awk '{print $1}')
	func=$(echo "$error_location"|awk '{print $2}')
	file=$(echo "$error_location"|awk '{print $3}')

	local path
	path=$(resolve_path "$file")

	dump_details \
		"${line}" \
		"${func}" \
		"${path}"

	exit 1
}

function warn() {
	local msg="$*"
	echo -e "[$(basename $0):${BASH_LINENO[0]}] WARNING: $msg"
}

function info() {
	local msg="$*"
	echo -e "[$(basename $0):${BASH_LINENO[0]}] INFO: $msg"
}

function bats_unbuffered_info() {
	local msg="$*"
	# Ask bats to print this text immediately rather than buffering until the end of a test case.
	echo -e "[$(basename $0):${BASH_LINENO[0]}] UNBUFFERED: INFO: $msg" >&3
}

function handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo -e "[$(basename $0):$line_number] ERROR: $(eval echo "$BASH_COMMAND")"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

# A wrapper function for kubectl with retry logic
# runs the command up to 5 times with a 15-second interval
# to ensure successful execution
function kubectl_retry() {
	local max_tries=5
	local interval=15
	local i=0
	while true; do
		kubectl $@ && return 0 || true
		i=$((i + 1))
		[ $i -lt $max_tries ] && echo "'kubectl $@' failed, retrying in $interval seconds" 1>&2 || break
		sleep $interval
	done
	echo "'kubectl $@' failed after $max_tries tries" 1>&2 && return 1
}

function waitForProcess() {
	wait_time="$1"
	sleep_time="$2"
	cmd="$3"
	while [ "$wait_time" -gt 0 ]; do
		if eval "$cmd"; then
			return 0
		else
			sleep "$sleep_time"
			wait_time=$((wait_time-sleep_time))
		fi
	done
	return 1
}

# Check if the $1 argument is the name of a 'known'
# Kata runtime. Of course, the end user can choose any name they
# want in reality, but this function knows the names of the default
# and recommended Kata docker runtime install names.
function is_a_kata_runtime() {
	if [ "$1" = "containerd-shim-kata-v2" ] || [ "$1" = "io.containerd.kata.v2" ]; then
		echo "1"
	else
		echo "0"
	fi
}

# Gets versions and paths of all the components
# list in kata-env
function extract_kata_env() {
	local cmd
	local config_path
	local runtime_version
	local runtime_version_semver
	local runtime_version_commit
	local runtime_path
	local hypervisor_path
	local virtiofsd_path
	local initrd_path
	local kata_env
	local req_memory_amount
	local req_num_vcpus

	case "${KATA_HYPERVISOR}" in
		dragonball)
			cmd=kata-ctl
			config_path=".runtime.config.path"
			runtime_version=".runtime.version"
			runtime_version_semver="semver"
			runtime_version_commit="commit"
			runtime_path=".runtime.path"
			hypervisor_path=".hypervisor.path"
			virtio_fs_daemon_path=".hypervisor.virtio_fs_daemon"
			initrd_path=".initrd.path"
			shared_fs=".hypervisor.shared_fs"
			;;
		*)
			cmd=kata-runtime
			config_path=".Runtime.Config.Path"
			runtime_version=".Runtime.Version"
			runtime_version_semver="Semver"
			runtime_version_commit="Commit"
			runtime_path=".Runtime.Path"
			hypervisor_path=".Hypervisor.Path"
			virtio_fs_daemon_path=".Hypervisor.VirtioFSDaemon"
			initrd_path=".Initrd.Path"
			shared_fs=".Hypervisor.SharedFS"
			req_memory_amount=".Host.Memory.Total"
			req_num_vcpus=""
			;;
	esac
	kata_env="$(sudo ${cmd} env --json)"

	RUNTIME_CONFIG_PATH="$(echo "${kata_env}" | jq -r ${config_path})"
	RUNTIME_VERSION="$(echo "${kata_env}" | jq -r ${runtime_version} | grep ${runtime_version_semver} | cut -d'"' -f4)"
	RUNTIME_COMMIT="$(echo "${kata_env}" | jq -r ${runtime_version} | grep ${runtime_version_commit} | cut -d'"' -f4)"
	RUNTIME_PATH="$(echo "${kata_env}" | jq -r ${runtime_path})"
	SHARED_FS="$(echo "${kata_env}" | jq -r ${shared_fs})"

	# get the requested memory and num of vcpus from the kata config file.
	config_content="$(cat ${RUNTIME_CONFIG_PATH} | grep -vE "^#")"
	REQ_MEMORY="$(echo "${config_content}" | grep -i 'default_memory =' | cut -d  "=" -f2 | awk '{print $1}')"
	REQ_NUM_VCPUS="$(echo "${config_content}" | grep -i 'default_vcpus =' | cut -d  "=" -f2 | awk '{print $1}')"

	# Shimv2 path is being affected by https://github.com/kata-containers/kata-containers/issues/1151
	SHIM_PATH=$(command -v containerd-shim-kata-v2)
	[ -L ${SHIM_PATH} ] && SHIM_PATH=$(readlink ${SHIM_PATH})

	SHIM_VERSION=${RUNTIME_VERSION}

	HYPERVISOR_PATH=$(echo "${kata_env}" | jq -r ${hypervisor_path})
	VIRTIOFSD_PATH=$(echo "${kata_env}" | jq -r ${virtio_fs_daemon_path})
	INITRD_PATH=$(echo "${kata_env}" | jq -r ${initrd_path})

	# TODO: there is no ${cmd} of rust version currently
	if [ "${KATA_HYPERVISOR}" != "dragonball" ]; then
		if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
			HYPERVISOR_VERSION=$(sudo -E ${HYPERVISOR_PATH} -version | head -n1)
		else
			HYPERVISOR_VERSION=$(sudo -E ${HYPERVISOR_PATH} --version | head -n1)
		fi
	fi
}

# Checks that processes are not running
function check_processes() {
	extract_kata_env

	# Only check the kata-env if we have managed to find the kata executable...
	if [ -x "$RUNTIME_PATH" ]; then
		local vsock_configured=$($RUNTIME_PATH env | awk '/UseVSock/ {print $3}')
		local vsock_supported=$($RUNTIME_PATH env | awk '/SupportVSock/ {print $3}')
	else
		local vsock_configured="false"
		local vsock_supported="false"
	fi

	general_processes=( ${HYPERVISOR_PATH} ${SHIM_PATH} )

	for i in "${general_processes[@]}"; do
		if pgrep -f "$i"; then
			die "Found unexpected ${i} present"
		fi
	done
}

# Clean environment, this function will try to remove all
# stopped/running containers.
function clean_env()
{
	# If the timeout has not been set, default it to 30s
	# Docker has a built in 10s default timeout, so make ours
	# longer than that.
	KATA_DOCKER_TIMEOUT=${KATA_DOCKER_TIMEOUT:-30}
	containers_running=$(sudo timeout ${KATA_DOCKER_TIMEOUT} docker ps -q)

	if [ ! -z "$containers_running" ]; then
		# First stop all containers that are running
		# Use kill, as the containers are generally benign, and most
		# of the time our 'stop' request ends up doing a `kill` anyway
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker kill $containers_running

		# Remove all containers
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker rm -f $(docker ps -qa)
	fi
}

function clean_env_ctr()
{
	local count_running="$(sudo ctr c list -q | wc -l)"
	local remaining_attempts=10
	declare -a running_tasks=()
	local count_tasks=0
	local sleep_time=1
	local time_out=10

	[ "$count_running" -eq "0" ] && return 0

	readarray -t running_tasks < <(sudo ctr t list -q)

	info "Wait until the containers gets removed"

	for task_id in "${running_tasks[@]}"; do
		sudo timeout -s SIGKILL 30s ctr t kill -a -s SIGKILL ${task_id} >/dev/null 2>&1 || true
		sleep 0.5
	done

	# do not stop if the command fails, it will be evaluated by waitForProcess
	local cmd="[[ $(sudo ctr tasks list | grep -c "STOPPED") == "$count_running" ]]" || true

	local res="ok"
	waitForProcess "${time_out}" "${sleep_time}" "$cmd" || res="fail"

	[ "$res" == "ok" ] || sudo systemctl restart containerd

	while (( remaining_attempts > 0 )); do
		[ "${RUNTIME}" == "runc" ] && sudo ctr tasks rm -f $(sudo ctr task list -q)
		sudo ctr c rm $(sudo ctr c list -q) >/dev/null 2>&1

		count_running="$(sudo ctr c list -q | wc -l)"

		[ "$count_running" -eq 0 ] && break

		remaining_attempts=$((remaining_attempts-1))
		sleep 0.5
	done

	count_tasks="$(sudo ctr t list -q | wc -l)"

	if (( count_tasks > 0 )); then
		die "Can't remove running containers."
	fi
}

# Kills running shim and hypervisor components
function kill_kata_components() {
	local ATTEMPTS=2
	local TIMEOUT="30s"
	local PID_NAMES=( "containerd-shim-kata-v2" "qemu-system-x86_64" "qemu-system-x86_64-tdx-experimental" "cloud-hypervisor" )

	sudo systemctl stop containerd
	# iterate over the list of kata components and stop them
	for (( i=1; i<=ATTEMPTS; i++ )); do
		for PID_NAME in "${PID_NAMES[@]}"; do
			[[ ! -z "$(pidof ${PID_NAME})" ]] && sudo killall -w -s SIGKILL "${PID_NAME}" >/dev/null 2>&1 || true
		done
		sleep 1
	done
	sudo timeout -s SIGKILL "${TIMEOUT}" systemctl start containerd
}

# Restarts a systemd service while ensuring the start-limit-burst is set to 0.
# Outputs warnings to stdio if something has gone wrong.
#
# Returns 0 on success, 1 otherwise
function restart_systemd_service_with_no_burst_limit() {
	local service=$1
	info "restart $service service"

	local active=$(systemctl show "$service.service" -p ActiveState | cut -d'=' -f2)
	[ "$active" == "active" ] || warn "Service $service is not active"

	local start_burst=$(systemctl show "$service".service -p StartLimitBurst | cut -d'=' -f2)
	if [ "$start_burst" -ne 0 ]
	then
		local unit_file=$(systemctl show "$service.service" -p FragmentPath | cut -d'=' -f2)
		[ -f "$unit_file" ] || { warn "Can't find $service's unit file: $unit_file"; return 1; }

		# If the unit file is in /lib, copy it to /etc
		if [[ $unit_file == /lib* ]]; then
			tmp_unit_file="/etc/${unit_file#*lib/}"
			sudo cp "$unit_file" "$tmp_unit_file"
			unit_file="$tmp_unit_file"
		fi

		local start_burst_set=$(sudo grep StartLimitBurst $unit_file | wc -l)
		if [ "$start_burst_set" -eq 0 ]
		then
			sudo sed -i '/\[Service\]/a StartLimitBurst=0' "$unit_file"
		else
			sudo sed -i 's/StartLimitBurst.*$/StartLimitBurst=0/g' "$unit_file"
		fi

		sudo systemctl daemon-reload
	fi

	sudo systemctl restart "$service"

	local state=$(systemctl show "$service.service" -p SubState | cut -d'=' -f2)
	[ "$state" == "running" ] || { warn "Can't restart the $service service"; return 1; }

	start_burst=$(systemctl show "$service.service" -p StartLimitBurst | cut -d'=' -f2)
	[ "$start_burst" -eq 0 ] || { warn "Can't set start burst limit for $service service"; return 1; }

	return 0
}

function restart_containerd_service() {
	restart_systemd_service_with_no_burst_limit containerd || return 1

	local retries=5
	local counter=0
	until [ "$counter" -ge "$retries" ] || sudo ctr --connect-timeout 1s version > /dev/null 2>&1
	do
		info "Waiting for containerd socket..."
		((counter++))
	done

	[ "$counter" -ge "$retries" ] && { warn "Can't connect to containerd socket"; return 1; }

	clean_env_ctr
	return 0
}

function restart_crio_service() {
	sudo systemctl restart crio
}

# Configures containerd
function overwrite_containerd_config() {
	containerd_config="/etc/containerd/config.toml"
	sudo rm -f "${containerd_config}"
	sudo tee "${containerd_config}" << EOF
version = 2

[plugins]
  [plugins."io.containerd.grpc.v1.cri"]
    [plugins."io.containerd.grpc.v1.cri".containerd]
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
          base_runtime_spec = ""
          cni_conf_dir = ""
          cni_max_conf_num = 0
          container_annotations = []
          pod_annotations = []
          privileged_without_host_devices = false
          runtime_engine = ""
          runtime_path = ""
          runtime_root = ""
          runtime_type = "io.containerd.runc.v2"
          [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
            BinaryName = ""
            CriuImagePath = ""
            CriuPath = ""
            CriuWorkPath = ""
            IoGid = 0
            IoUid = 0
            NoNewKeyring = false
            NoPivotRoot = false
            Root = ""
            ShimCgroup = ""
            SystemdCgroup = false
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
          runtime_type = "io.containerd.kata.v2"
EOF
}

# Configures CRI-O
function overwrite_crio_config() {
	crio_conf_d="/etc/crio/crio.conf.d"
	sudo mkdir -p ${crio_conf_d}

	kata_config="${crio_conf_d}/99-kata-containers"
	sudo tee "${kata_config}" << EOF
[crio.runtime.runtimes.kata]
runtime_path = "/usr/local/bin/containerd-shim-kata-v2"
runtime_type = "vm"
runtime_root = "/run/vc"
runtime_config_path = "/opt/kata/share/defaults/kata-containers/configuration.toml"
privileged_without_host_devices = true
EOF

	debug_config="${crio_conf_d}/100-debug"
	sudo tee "${debug_config}" << EOF
[crio]
log_level = "debug"
EOF
}

function install_kata_core() {
	declare -r katadir="$1"
	declare -r destdir="/"
	declare -r kata_tarball="kata-static.tar.xz"

	# Removing previous kata installation
	sudo rm -rf "${katadir}"

	pushd "${kata_tarball_dir}"
	sudo tar -xvf "${kata_tarball}" -C "${destdir}"
	popd
}

function install_kata_tools() {
	declare -r katadir="/opt/kata"

	# TODO: implement a better way to install the tools - see issue #8864.
	install_kata_core "${katadir}"
}

function install_kata() {
	declare -r katadir="/opt/kata"
	declare -r local_bin_dir="/usr/local/bin/"

	install_kata_core "${katadir}"

	# create symbolic links to kata components
	for b in "${katadir}"/bin/* ; do
		sudo ln -sf "${b}" "${local_bin_dir}/$(basename $b)"
	done

	if [ "${CONTAINER_ENGINE:=containerd}" = "containerd" ]; then
		check_containerd_config_for_kata
		restart_containerd_service
	else
		overwrite_crio_config
		restart_crio_service
	fi

	load_vhost_mods
}

# creates a new kata configuration.toml hard link that
# points to the hypervisor passed by KATA_HYPERVISOR env var.
function enabling_hypervisor() {
	declare -r KATA_DIR="/opt/kata"
	declare -r CONTAINERD_SHIM_KATA="/usr/local/bin/containerd-shim-kata-${KATA_HYPERVISOR}-v2"

	case "${KATA_HYPERVISOR}" in
		dragonball|cloud-hypervisor|qemu-runtime-rs)
			sudo ln -sf "${KATA_DIR}/runtime-rs/bin/containerd-shim-kata-v2" "${CONTAINERD_SHIM_KATA}"
			declare -r CONFIG_DIR="${KATA_DIR}/share/defaults/kata-containers/runtime-rs"
			;;
		*)
			sudo ln -sf "${KATA_DIR}/bin/containerd-shim-kata-v2" "${CONTAINERD_SHIM_KATA}"
			declare -r CONFIG_DIR="${KATA_DIR}/share/defaults/kata-containers"
			;;
	esac

	declare -r SRC_HYPERVISOR_CONFIG="${CONFIG_DIR}/configuration-${KATA_HYPERVISOR}.toml"
	declare -r DEST_KATA_CONFIG="${CONFIG_DIR}/configuration.toml"

	sudo ln -sf "${SRC_HYPERVISOR_CONFIG}" "${DEST_KATA_CONFIG}"

	export KATA_CONFIG_PATH="${DEST_KATA_CONFIG}"
}


function check_containerd_config_for_kata() {
	# check containerd config
	declare -r line1="default_runtime_name = \"kata\""
	declare -r line2="runtime_type = \"io.containerd.kata.v2\""
	declare -r num_lines_containerd=2
	declare -r containerd_path="/etc/containerd/config.toml"
	local count_matches=$(grep -ic  "$line1\|$line2" "${containerd_path}")

	if [ "${count_matches}" = "${num_lines_containerd}" ]; then
		info "containerd ok"
	else
		info "overwriting containerd configuration w/ a valid one"
		overwrite_containerd_config
	fi
}

function ensure_yq() {
	: "${GOPATH:=${GITHUB_WORKSPACE:-$HOME/go}}"
	export GOPATH
	export PATH="${GOPATH}/bin:${PATH}"
	INSTALL_IN_GOPATH=true "${repo_root_dir}/ci/install_yq.sh"
	hash -d yq 2> /dev/null || true # yq is preinstalled on GHA Ubuntu 22.04 runners so we clear Bash's PATH cache.
}

function ensure_helm() {
	ensure_yq
	# The get-helm-3 script will take care of downloaading and installing Helm
	# properly on the system respecting ARCH, OS and other configurations.
	DESIRED_VERSION=$(get_from_kata_deps ".externals.helm.version")
	export DESIRED_VERSION

	# Check if helm is available in the system's PATH
	if ! command -v helm &> /dev/null; then
		echo "Helm is not installed. Installing Helm..."
		curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
		# Verify the installation
		if command -v helm &> /dev/null; then
			echo "Helm installed successfully."
		else
			echo "Failed to install Helm."
			exit 1
		fi
	else
		echo "Helm is already installed."
	fi
}

# dependency: What we want to get the version from the versions.yaml file
function get_from_kata_deps() {
        versions_file="${repo_root_dir}/versions.yaml"

        command -v yq &>/dev/null || die 'yq command is not in your $PATH'

        yq_version=$(yq --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | cut -d. -f1)
        if [ "$yq_version" -eq 3 ]; then
          dependency=$(echo "$1" | sed "s/^\.//g")
          result=$("yq" read "$versions_file" "$dependency")
        else
          dependency=$1
          result=$("yq" "$dependency | explode (.)" "$versions_file")
        fi

        [ "$result" = "null" ] && result=""
        echo "$result"
}

# project: org/repo format
# base_version: ${major}.${minor}
function get_latest_patch_release_from_a_github_project() {
       project="${1}"
       base_version="${2}"

       curl --silent https://api.github.com/repos/${project}/releases | jq -r .[].tag_name | grep "^${base_version}.[0-9]*$" -m1
}

# base_version: The version to be intalled in the ${major}.${minor} format
function clone_cri_containerd() {
	base_version="${1}"

	project="containerd/containerd"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	rm -rf containerd
	git clone -b ${version} https://github.com/${project}
}

# project: org/repo format
# version: the version of the tarball that will be downloaded
# tarball-name: the name of the tarball that will be downloaded
function download_github_project_tarball() {
	project="${1}" 
	version="${2}"
	tarball_name="${3}"

	wget https://github.com/${project}/releases/download/${version}/${tarball_name}
}

# version: The version to be intalled
function install_cni_plugins() {
	version="${1}"

	project="containernetworking/plugins"
	tarball_name="cni-plugins-linux-$(${repo_root_dir}/tests/kata-arch.sh -g)-${version}.tgz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo mkdir -p /opt/cni/bin
	sudo tar -xvf "${tarball_name}" -C /opt/cni/bin
	rm -f "${tarball_name}"

	cni_config="/etc/cni/net.d/10-containerd-net.conflist"
	if [ ! -f ${cni_config} ];then
		sudo mkdir -p /etc/cni/net.d
		sudo tee "${cni_config}" << EOF
{
  "cniVersion": "1.0.0",
  "name": "containerd-net",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cni0",
      "isGateway": true,
      "ipMasq": true,
      "promiscMode": true,
      "ipam": {
        "type": "host-local",
        "ranges": [
          [{
            "subnet": "10.88.0.0/16"
          }],
          [{
            "subnet": "2001:4860:4860::/64"
          }]
        ],
        "routes": [
          { "dst": "0.0.0.0/0" },
          { "dst": "::/0" }
        ]
      }
    },
    {
      "type": "portmap",
      "capabilities": {"portMappings": true}
    }
  ]
}
EOF
	fi
}

# version: The version to be installed
function install_runc() {
	base_version="${1}"
	project="opencontainers/runc"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	if [ -f /usr/local/sbin/runc ]; then
		return
	fi

	binary_name="runc.$(${repo_root_dir}/tests/kata-arch.sh -g)"
	download_github_project_tarball "${project}" "${version}" "${binary_name}"

	sudo mkdir -p /usr/local/sbin
	sudo mv $binary_name /usr/local/sbin/runc
	sudo chmod +x /usr/local/sbin/runc
}

# base_version: The version to be intalled in the ${major}.${minor} format
function install_cri_containerd() {
	base_version="${1}"

	project="containerd/containerd"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	tarball_name="containerd-${version//v}-linux-$(${repo_root_dir}/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	#add the "--keep-directory-symlink" option to make sure the untar wouldn't override the
	#system rootfs's bin/sbin directory which would be a symbol link to /usr/bin or /usr/sbin.
	if [ ! -f /usr/local ]; then
		sudo mkdir -p /usr/local
	fi
	sudo tar --keep-directory-symlink -xvf "${tarball_name}" -C /usr/local/
	rm -f "${tarball_name}"

	sudo mkdir -p /etc/containerd
	containerd config default | sudo tee /etc/containerd/config.toml

	containerd_service="/etc/systemd/system/containerd.service"

	if [ ! -f ${containerd_service} ]; then
		sudo mkdir -p /etc/systemd/system
		sudo tee ${containerd_service}  <<EOF
[Unit]
Description=containerd container runtime
Documentation=https://containerd.io
After=network.target local-fs.target

[Service]
ExecStartPre=-/sbin/modprobe overlay
ExecStart=/usr/local/bin/containerd

Type=notify
Delegate=yes
KillMode=process
Restart=always
RestartSec=5
# Having non-zero Limit*s causes performance problems due to accounting overhead
# in the kernel. We recommend using cgroups to do container-local accounting.
LimitNPROC=infinity
LimitCORE=infinity
LimitNOFILE=infinity
# Comment TasksMax if your systemd version does not supports it.
# Only systemd 226 and above support this version.
TasksMax=infinity
OOMScoreAdjust=-999

[Install]
WantedBy=multi-user.target
EOF
	fi 
}

# base_version: The version to be intalled in the ${major}.${minor} format
function install_cri_tools() {
	base_version="${1}"

	project="kubernetes-sigs/cri-tools"
	version=$(get_latest_patch_release_from_a_github_project "${project}" "${base_version}")

	tarball_name="crictl-${version}-linux-$(${repo_root_dir}/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar -xvf "${tarball_name}" -C /usr/local/bin
	rm -f "${tarball_name}"
}

function install_nydus() {
	version="${1}"

	project="dragonflyoss/image-service"
	tarball_name="nydus-static-${version}-linux-$(${repo_root_dir}/tests/kata-arch.sh -g).tgz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar xfz "${tarball_name}" -C /usr/local/bin --strip-components=1
	rm -f "${tarball_name}"
}

function install_nydus_snapshotter() {
	version="${1}"

	project="containerd/nydus-snapshotter"
	tarball_name="nydus-snapshotter-${version}-$(uname -s| tr A-Z a-z)-$(${repo_root_dir}/tests/kata-arch.sh -g).tar.gz"

	download_github_project_tarball "${project}" "${version}" "${tarball_name}"
	sudo tar xfz "${tarball_name}" -C /usr/local/bin --strip-components=1
	rm -f "${tarball_name}"
}

# version: the CRI-O version to be installe
function install_crio() {
	local version=${1}

	sudo mkdir -p /etc/apt/keyrings
	sudo mkdir -p /etc/apt/sources.list.d

	curl -fsSL https://pkgs.k8s.io/addons:/cri-o:/stable:/v${version}/deb/Release.key | \
		sudo gpg --dearmor -o /etc/apt/keyrings/cri-o-apt-keyring.gpg
	echo "deb [signed-by=/etc/apt/keyrings/cri-o-apt-keyring.gpg] https://pkgs.k8s.io/addons:/cri-o:/stable:/v${version}/deb/ /" | \
		sudo tee /etc/apt/sources.list.d/cri-o.list

	sudo apt update
	sudo apt install -y cri-o

	# We need to set the default capabilities to ensure our tests will pass
	# See: https://github.com/kata-containers/kata-containers/issues/8034
	sudo mkdir -p /etc/crio/crio.conf.d/
	cat <<EOF | sudo tee /etc/crio/crio.conf.d/00-default-capabilities
[crio]
storage_option = [
	"overlay.skip_mount_home=true",
]
[crio.runtime]
default_capabilities = [
       "CHOWN",
       "DAC_OVERRIDE",
       "FSETID",
       "FOWNER",
       "SETGID",
       "SETUID",
       "SETPCAP",
       "NET_BIND_SERVICE",
       "KILL",
       "SYS_CHROOT",
]
EOF

	sudo systemctl enable --now crio
}

function install_docker() {
	# Add Docker's official GPG key
	sudo apt-get update
	sudo apt-get -y install ca-certificates curl gnupg
	sudo install -m 0755 -d /etc/apt/keyrings
	curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
	sudo chmod a+r /etc/apt/keyrings/docker.gpg

	# Add the repository to Apt sources:
	echo \
		"deb [arch="$(dpkg --print-architecture)" signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
		"$(. /etc/os-release && echo "$VERSION_CODENAME")" stable" | \
		sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
	sudo apt-get update

	sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
}

# Convert architecture to the name used by golang
function arch_to_golang() {
	local arch="$(uname -m)"

	case "${arch}" in
		aarch64) echo "arm64";;
		ppc64le) echo "${arch}";;
		x86_64) echo "amd64";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Convert architecture to the name used by rust
function arch_to_rust() {
	local -r arch="$(uname -m)"

	case "${arch}" in
		aarch64) echo "${arch}";;
		ppc64le) echo "powerpc64le";;
		x86_64) echo "${arch}";;
		s390x) echo "${arch}";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Convert architecture to the name used by the Linux kernel build system
function arch_to_kernel() {
	local -r arch="$(uname -m)"

	case "${arch}" in
		aarch64) echo "arm64";;
		ppc64le) echo "powerpc";;
		x86_64) echo "${arch}";;
		s390x) echo "s390x";;
		*) die "unsupported architecture: ${arch}";;
	esac
}

# Obtain a list of the files the PR changed.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details_full()
{
        # List of filters used to restrict the types of file changes.
        # See git-diff-tree(1) for further info.
        local filters=""

        # Added file
        filters+="A"

        # Copied file
        filters+="C"

        # Modified file
        filters+="M"

        # Renamed file
        filters+="R"

        git diff-tree \
                -r \
                --name-status \
                --diff-filter="${filters}" \
                "origin/${branch}" HEAD
}

# Obtain a list of the files the PR changed, ignoring vendor files.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details()
{
        get_pr_changed_file_details_full | grep -v "vendor/"
}

function get_dep_from_yaml_db(){
        local versions_file="$1"
        local dependency="$2"

        [ ! -f "$versions_file" ] && die "cannot find $versions_file"

        "${repo_root_dir}/ci/install_yq.sh" >&2

        result=$("${GOPATH}/bin/yq" "$dependency" "$versions_file")
        [ "$result" = "null" ] && result=""
        echo "$result"
}

function get_test_version(){
        local dependency="$1"

        local db
        local cidir

        # directory of this script, not the caller
        local cidir=$(dirname "${BASH_SOURCE[0]}")

        db="${cidir}/../versions.yaml"

        get_dep_from_yaml_db "${db}" ".${dependency}"
}

# Load vhost, vhost_net, vhost_vsock modules.
function load_vhost_mods() {
	sudo modprobe vhost
	sudo modprobe vhost_net
	sudo modprobe vhost_vsock
}

function run_static_checks()
{
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$this_script_dir/static-checks.sh" "$@"
}

function run_docs_url_alive_check()
{
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$this_script_dir/static-checks.sh" --docs --all "github.com/kata-containers/kata-containers"
}

function run_get_pr_changed_file_details()
{
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	get_pr_changed_file_details
}

# Check if the 1st argument version is greater than and equal to 2nd one
# Version format: [0-9]+ separated by period (e.g. 2.4.6, 1.11.3 and etc.)
#
# Parameters:
#	$1	- a version to be tested
#	$2	- a target version
#
# Return:
# 	0 if $1 is greater than and equal to $2
#	1 otherwise
function version_greater_than_equal() {
	local current_version=$1
	local target_version=$2
	smaller_version=$(echo -e "$current_version\n$target_version" | sort -V | head -1)
	if [ "${smaller_version}" = "${target_version}" ]; then
		return 0
	else
		return 1
	fi
}
