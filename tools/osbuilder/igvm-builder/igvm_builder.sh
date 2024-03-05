#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o pipefail
set -o errtrace

[[ -n "${DEBUG:-}" ]] && set -x

SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
LIB_FILE="${SCRIPT_DIR}/../scripts/lib.sh"
# shellcheck source=tools/osbuilder/scripts/lib.sh
source "${LIB_FILE}"

# distro-specific config file
typeset -r CONFIG_SH="config.sh"

# Name of an optional distro-specific file which, if it exists, must implement the
# install_igvm_tool, build_igvm_files, and uninstall_igvm_tool functions.
typeset -r LIB_SH="igvm_lib.sh"

load_config_distro()
{
	distro_config_dir="${SCRIPT_DIR}/${DISTRO}"

	[[ -d "${distro_config_dir}" ]] || die "Could not find configuration directory '${distro_config_dir}'"

	if [[ -e "${distro_config_dir}/${LIB_SH}" ]]; then
		igvm_lib="${distro_config_dir}/${LIB_SH}"
		echo "igvm_lib.sh file found. Loading content"
		# shellcheck source=tools/osbuilder/igvm-builder/azure-linux/igvm_lib.sh
		source "${igvm_lib}"
	fi

	# Source config.sh from distro, depends on root_hash based variables here
	igvm_config="${distro_config_dir}/${CONFIG_SH}"
	# shellcheck source=tools/osbuilder/igvm-builder/azure-linux/config.sh
	source "${igvm_config}"
}

DISTRO="azure-linux"
MODE="build"

while getopts ":o:s:iu" OPTIONS; do
	case "${OPTIONS}" in
		o ) OUT_DIR=${OPTARG} ;;
		s ) SVN=${OPTARG} ;;
		i ) MODE="install" ;;
		u ) MODE="uninstall" ;;
		\? )
			echo "Error - Invalid Option: -${OPTARG}" 1>&2
			exit 1
			;;
		: )
			echo "Error - Invalid Option: -${OPTARG} requires an argument" 1>&2
			exit 1
			;;
  esac
done

echo "IGVM builder script"
echo "-- OUT_DIR -> ${OUT_DIR:-}"
echo "-- SVN -> ${SVN:-}"
echo "-- DISTRO -> ${DISTRO}"
echo "-- MODE -> ${MODE}"

if [[ -n "${DISTRO}" ]]; then
	load_config_distro
else
	echo "DISTRO must be specified"
	exit 1
fi

case "${MODE}" in
	"install")
		install_igvm_tool
		;;
	"uninstall")
		uninstall_igvm_tool
		;;
	"build")
		build_igvm_files
		;;
esac
