#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly script_dir=$(dirname $(readlink -f "$0"))
readonly script_name="$(basename "${BASH_SOURCE[0]}")"
# Source to trap error line number
# shellcheck source=../lib/common.bash
source "${script_dir}/../lib/common.bash"
# shellcheck source=./lib.sh
source "${script_dir}/lib.sh"

option="sandbox_cgroup_only"
request=${1:-}

usage(){
	cat <<EOT
Usage:
${script_name} <true|false|clean>"

false: Disable ${option}
true:  Enable ${option}

The configuration changes are applied in kata user config:
${KATA_ETC_CONFIG_PATH}

Remove it if you want to use the stateless options.
EOT
}

case ${request} in
	true)
		;;
	false)
		;;
	*)
		usage
		exit 1
		;;
esac

current_value=$(kata-runtime kata-env --json | jq ".Runtime.SandboxCgroupOnly")
if [ "$current_value" == "${request}" ]; then
	info "already ${request}"
	exit 0
fi

kata_config_path=$(kata-runtime kata-env --json | jq -r .Runtime.Config.Path)

bk_suffix="${option}-bk"


if [ -f "${KATA_ETC_CONFIG_PATH}" ] && [ "${KATA_ETC_CONFIG_PATH}" != ${kata_config_path} ]; then
	bk_file="${KATA_ETC_CONFIG_PATH}-${bk_suffix}"
	info "backup ${KATA_ETC_CONFIG_PATH} in ${bk_file}"
	sudo cp "${KATA_ETC_CONFIG_PATH}" "${bk_file}"
fi

if [ "${KATA_ETC_CONFIG_PATH}" != "${kata_config_path}" ]; then
	info "Creating etc config based on ${kata_config_path}"
	sudo cp "${kata_config_path}" "${KATA_ETC_CONFIG_PATH}"
fi

info "modifying config file : ${KATA_ETC_CONFIG_PATH}"
sudo crudini --set "${KATA_ETC_CONFIG_PATH}" "runtime" "${option}" "${request}"

info "Validate option is ${request}"
current_value=$(kata-runtime kata-env --json | jq ".Runtime.SandboxCgroupOnly")

[ "$current_value" == "${request}" ] || die "The option was not updated"

info "OK"
