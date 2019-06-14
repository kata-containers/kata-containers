#!/usr/bin/env bash
#
# Copyright (c) 2017-2018 Intel Corporation
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0
#

export KATA_RUNTIME=${KATA_RUNTIME:-kata-runtime}
export KATA_KSM_THROTTLER=${KATA_KSM_THROTTLER:-no}
export KATA_NEMU_DESTDIR=${KATA_NEMU_DESTDIR:-"/usr/local"}

# Name of systemd service for the throttler
KATA_KSM_THROTTLER_JOB="kata-ksm-throttler"

# How long do we wait for docker to perform a task before we
# timeout with the presumption it has hung.
# Docker itself has a 10s timeout, so make our timeout longer
# than that. Measured in seconds by default (see timeout(1) for
# more formats).
export KATA_DOCKER_TIMEOUT=30

# Number of seconds to wait for a general network operation to complete.
export KATA_NET_TIMEOUT=30

# Ensure GOPATH set
if command -v go > /dev/null; then
	export GOPATH=${GOPATH:-$(go env GOPATH)}
else
	# if go isn't installed, set default location for GOPATH
	export GOPATH="${GOPATH:-$HOME/go}"
fi

tests_repo="${tests_repo:-github.com/kata-containers/tests}"
lib_script="${GOPATH}/src/${tests_repo}/lib/common.bash"
source "${lib_script}"

export KATA_OBS_REPO_BASE="http://download.opensuse.org/repositories/home:/katacontainers:/releases:/$(arch):/master"

# If we fail for any reason a message will be displayed
die() {
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

info() {
	echo -e "INFO: $*"
}

function build_version() {
	github_project="$1"
	make_target="$2"
	version="$3"

	[ -z "${version}" ] && die "need version to build"

	project_dir="${GOPATH}/src/${github_project}"

	[ -d "${project_dir}" ] || go get -d "${github_project}" || true

	pushd "${project_dir}"

	if [ "$version" != "HEAD" ]; then
		info "Using ${github_project} version ${version}"
		git checkout -b "${version}" "${version}"
	fi

	info "Building ${github_project}"
	if [ ! -f Makefile ]; then
		if [ -f autogen.sh ]; then
			info "Run autogen.sh to generate Makefile"
			bash -f autogen.sh
		fi
	fi

	if [ -f Makefile ]; then
		make ${make_target}
	else
		# install locally (which is what "go get" does by default)
		go install ./...
	fi

	popd
}

function build() {
	github_project="$1"
	make_target="$2"

	build_version "${github_project}" "${make_target}" "HEAD"
}

function build_and_install() {
	github_project="$1"
	make_target="$2"
	test_not_gopath_set="$3"

	build "${github_project}" "${make_target}"
	pushd "${GOPATH}/src/${github_project}"
	if [ "$test_not_gopath_set" = "true" ]; then
		info "Installing ${github_project} in No GO command or GOPATH not set mode"
		sudo -E PATH="$PATH" KATA_RUNTIME="${KATA_RUNTIME}" make install
		[ $? -ne 0 ] && die "Fail to install ${github_project} in No GO command or GOPATH not set mode"
	fi
	info "Installing ${github_project}"
	sudo -E PATH="$PATH" KATA_RUNTIME="${KATA_RUNTIME}" make install
	popd
}

function get_dep_from_yaml_db(){
	local versions_file="$1"
	local dependency="$2"

	[ ! -f "$versions_file" ] && die "cannot find $versions_file"

	# directory of this script, not the caller
	local cidir=$(dirname "${BASH_SOURCE[0]}")

	${cidir}/install_yq.sh >&2

	result=$("${GOPATH}/bin/yq" read "$versions_file" "$dependency")
	[ "$result" = "null" ] && result=""
	echo "$result"
}

function get_version(){
	dependency="$1"
	runtime_repo="github.com/kata-containers/runtime"
	runtime_repo_dir="$GOPATH/src/${runtime_repo}"
	versions_file="${runtime_repo_dir}/versions.yaml"
	mkdir -p "$(dirname ${runtime_repo_dir})"
	[ -d "${runtime_repo_dir}" ] ||  git clone --quiet https://${runtime_repo}.git "${runtime_repo_dir}"

	get_dep_from_yaml_db "${versions_file}" "${dependency}"
}

function get_test_version(){
	local dependency="$1"

	local db
	local cidir

	# directory of this script, not the caller
	local cidir=$(dirname "${BASH_SOURCE[0]}")

	db="${cidir}/../versions.yaml"

	get_dep_from_yaml_db "${db}" "${dependency}"
}

function waitForProcess(){
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

kill_stale_process()
{
	clean_env
	extract_kata_env
	stale_process_union=( "${stale_process_union[@]}" "${PROXY_PATH}" "${HYPERVISOR_PATH}" "${SHIM_PATH}" )
	for stale_process in "${stale_process_union[@]}"; do
		local pids=$(pgrep -d ' ' -f "${stale_process}")
		if [ -n "$pids" ]; then
			sudo kill -9 ${pids} || true
		fi
	done
}

delete_stale_docker_resource()
{
	local docker_status=false
	# check if docker service is running
	systemctl is-active --quiet docker
	if [ $? -eq 0 ]; then
		docker_status=true
		sudo systemctl stop docker
	fi
	# before removing stale docker dir, you should umount related resource
	for stale_docker_mount_point in "${stale_docker_mount_point_union[@]}"; do
		local mount_point_union=$(mount | grep "${stale_docker_mount_point}" | awk '{print $3}')
		if [ -n "${mount_point_union}" ]; then
			while IFS='$\n' read mount_point; do
				[ -n "$(grep "${mount_point}" "/proc/mounts")" ] && sudo umount -R "${mount_point}"
			done <<< "${mount_point_union}"
		fi
	done
	# remove stale docker dir
	for stale_docker_dir in "${stale_docker_dir_union[@]}"; do
		if [ -d "${stale_docker_dir}" ]; then
			sudo rm -rf "${stale_docker_dir}"
		fi
	done
	[ "${docker_status}" = true ] && sudo systemctl restart docker
}

delete_stale_kata_resource()
{
	for stale_kata_dir in "${stale_kata_dir_union[@]}"; do
		if [ -d "${stale_kata_dir}" ]; then
			sudo rm -rf "${stale_kata_dir}"
		fi
	done
}

delete_kata_repo_registrations()
{
	case "$ID" in
		ubuntu)
			local apt_file="/etc/apt/sources.list.d/kata-containers.list"
			if [ -f "$apt_file" ]; then
				info "Removing Kata apt file [$apt_file]"
				sudo rm -f "$apt_file"
			fi

			sudo apt-key list | grep 'home:katacontainers' > /dev/null
			if [ $? -eq 0 ]; then
				# apt-key output format changed at ubuntu 16.10
				if [ "$VERSION_ID" \< "16.10" ]; then
					kata_uuid="$(sudo apt-key list | awk '$2=="home:katacontainers" {print prev} {prev=$2}')"
					kata_uuid="${kata_uuid##*/}"
				else
					kata_uuid="$(sudo apt-key list | awk '$4=="home:katacontainers" {print prev} {prev=$0}')"
				fi

				if [ -n "$kata_uuid" ]; then
					info "Removing Kata apt key [$kata_uuid]"
					sudo apt-key del "$kata_uuid"
				else
					die "Failed to parse apt-key output for [$ID][$VERSION_ID]"
				fi
			else
				info "No katacontainers key found - not removing"
			fi
			;;

		*) info "Do not know how to clean repos from distro [$ID]";;
	esac
}

gen_clean_arch() {
	# Set up some vars
	stale_process_union=( "docker-containerd-shim" )
	#docker supports different storage driver, such like overlay2, aufs, etc.
	docker_storage_driver=$(timeout ${KATA_DOCKER_TIMEOUT} docker info --format='{{.Driver}}')
	stale_docker_mount_point_union=( "/var/lib/docker/containers" "/var/lib/docker/${docker_storage_driver}" )
	stale_docker_dir_union=( "/var/lib/docker" )
	stale_kata_dir_union=( "/var/lib/vc" "/run/vc" "/usr/share/kata-containers" "/usr/share/defaults/kata-containers" )

	info "kill stale process"
	kill_stale_process
	info "delete stale docker resource under ${stale_docker_dir_union[@]}"
	delete_stale_docker_resource
	info "delete stale kata resource under ${stale_kata_dir_union[@]}"
	delete_stale_kata_resource
	info "Remove installed kata packages"
	${GOPATH}/src/${tests_repo}/cmd/kata-manager/kata-manager.sh remove-packages
	info "Remove installed kubernetes packages and configuration"
	if [ "$ID" == ubuntu ]; then
		sudo rm -rf /etc/systemd/system/kubelet.service.d
		sudo apt-get purge kubeadm kubelet kubectl -y
	fi
	info "Remove Kata package repo registrations"
	delete_kata_repo_registrations

	info "Clean GOCACHE"
	if command -v go > /dev/null; then
		GOCACHE=${GOCACHE:-$(go env GOCACHE)}
	else
		# if go isn't installed, try default dir
		GOCACHE=${GOCACHE:-$HOME/.cache/go-build}
	fi
	[ -d "$GOCACHE" ] && sudo rm -rf ${GOCACHE}/*
}

build_install_parallel() {
	gnu_parallel_url=$(get_test_version "externals.parallel.url")
	gnu_parallel_version=$(get_test_version "externals.parallel.version")
	gnu_parallel_tar_pkg="parallel-${gnu_parallel_version}.tar.bz2"
	gnu_parallel_dir="./${gnu_parallel_tar_pkg//.*}"

	chronic curl -sLO "${gnu_parallel_url}/${gnu_parallel_tar_pkg}"
	tar -xf "${gnu_parallel_tar_pkg}"

	pushd "${gnu_parallel_dir}"
	chronic ./configure
	chronic make
	chronic sudo make install
	popd

	rm -rf "$gnu_parallel_dir" "$gnu_parallel_tar_pkg"
}

check_git_version() {
	result="true"

        local required_version_major=$(echo "$1" | cut -d. -f1)
        local required_version_medium=$(echo "$1" | cut -d. -f2)
        local required_version_minor=$(echo "$1" | cut -d. -f3)

        local git_version=$(git version | cut -d' ' -f3)
        [ -n "${git_version}" ] || die "cannot determine git version, please ensure it is installed"

        local current_version_major=$(echo "${git_version}" | cut -d. -f1)
        local current_version_medium=$(echo "${git_version}" | cut -d. -f2)
        local current_version_minor=$(echo "${git_version}" | cut -d. -f3)

        [[ ${current_version_major} -lt ${required_version_major} ]] || \
        [[ ( ${current_version_major} -eq ${required_version_major} ) && ( ${current_version_medium} -lt ${required_version_medium} ) ]] || \
        [[ ( ${current_version_major} -eq ${required_version_major} ) && ( ${current_version_medium} -eq ${required_version_medium} ) && ( ${current_version_minor} -lt ${required_version_minor} ) ]] && \
        result="false"

	echo "${result}"
}
