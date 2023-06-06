#!/bin/bash
#Copyright (c) 2021 Intel Corporation
#
#SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
NOTEBOOK_PORT="8888"

results_dir=${1:-}

usage(){
	echo "$0 <results_dir>"
}

if [ "${results_dir}" == "" ];then
	echo "missing results directory"
	usage
	exit 1
fi

if [ ! -d "${results_dir}" ];then
	echo "${results_dir} is not a directory"
	usage
	exit 1
fi

results_dir=$(realpath "${results_dir}")

sudo -E docker run --rm -p "${NOTEBOOK_PORT}:${NOTEBOOK_PORT}" -e JUPYTER_ENABLE_LAB=yes \
	-v "${script_dir}:/home/jovyan" \
	-v "${results_dir}:/home/jovyan/results" \
	jupyter/scipy-notebook:399cbb986c6b \
	start.sh jupyter lab --LabApp.token=''
