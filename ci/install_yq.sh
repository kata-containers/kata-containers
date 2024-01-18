#!/usr/bin/env bash
#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

# If we fail for any reason a message will be displayed
die() {
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Install the yq yaml query package from the mikefarah github repo
# Install via binary download, as we may not have golang installed at this point
function install_yq() {
	local yq_pkg="github.com/mikefarah/yq"
	local yq_version=3.4.1
	local precmd=""
	INSTALL_IN_GOPATH=${INSTALL_IN_GOPATH:-true}

	if [ "${INSTALL_IN_GOPATH}"  == "true" ];then
		GOPATH=${GOPATH:-${HOME}/go}
		mkdir -p "${GOPATH}/bin"
		local yq_path="${GOPATH}/bin/yq"
	else
		yq_path="/usr/local/bin/yq"
		# Check if we need sudo to install yq
		if [ ! -w "/usr/local/bin" ]; then
			# Check if we have sudo privileges
			if ! sudo -n true 2>/dev/null; then
				die "Please provide sudo privileges to install yq"
			else
				precmd="sudo"
			fi
		fi
	fi
	[ -x  "${yq_path}" ] && [ "`${yq_path} --version`"X == "yq version ${yq_version}"X ] && return

	read -r -a sysInfo <<< "$(uname -sm)"

	case "${sysInfo[0]}" in
	"Linux" | "Darwin")
		goos="${sysInfo[0],}"
		;;
	"*")
		die "OS ${sysInfo[0]} not supported"
		;;
	esac

	case "${sysInfo[1]}" in
	"aarch64")
		goarch=arm64
		;;
	"arm64")
		# If we're on an apple silicon machine, just assign amd64. 
		# The version of yq we use doesn't have a darwin arm build, 
		# but Rosetta can come to the rescue here.
		if [ $goos == "Darwin" ]; then 
			goarch=amd64
		else 
			goarch=arm64
		fi
		;;
	"ppc64le")
		goarch=ppc64le
		;;
	"x86_64")
		goarch=amd64
		;;
	"s390x")
		goarch=s390x
		;;
	"*")
		die "Arch ${sysInfo[1]} not supported"
		;;
	esac


	# Check curl
	if ! command -v "curl" >/dev/null; then
		die "Please install curl"
	fi

	## NOTE: ${var,,} => gives lowercase value of var
	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_${goos}_${goarch}"
	${precmd} curl -o "${yq_path}" -LSsf "${yq_url}"
	[ $? -ne 0 ] && die "Download ${yq_url} failed"
	${precmd} chmod +x "${yq_path}"

	if ! command -v "${yq_path}" >/dev/null; then
		die "Cannot not get ${yq_path} executable"
	fi
}

install_yq
