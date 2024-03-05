#!/usr/bin/env bash
#
# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

# shellcheck disable=SC2034,SC2154

install_igvm_tool()
{
	echo "Installing IGVM tool"
	if [[ -d "${IGVM_EXTRACT_FOLDER}" ]]; then
		echo "${IGVM_EXTRACT_FOLDER} folder already exists, assuming tool is already installed"
		return
	fi

	# the igvm tool on Azure Linux will soon be properly installed through dnf via kata-packages-uvm-build
	# as of now, even when installing with pip3, we cannot delete the source folder as the ACPI tables are not being installed anywhere, hence relying on this folder
	echo "Determining and downloading latest IGVM tooling release, and extracting including ACPI tables"
	IGVM_VER=$(curl -sL "https://api.github.com/repos/microsoft/igvm-tooling/releases/latest" | jq -r .tag_name | sed 's/^v//')
	curl -sL "https://github.com/microsoft/igvm-tooling/archive/refs/tags/${IGVM_VER}.tar.gz" | tar --no-same-owner -xz
	mv "igvm-tooling-${IGVM_VER}" "${IGVM_EXTRACT_FOLDER}"

	echo "Installing IGVM module msigvm (${IGVM_VER}) via pip3"
	pushd "${IGVM_EXTRACT_FOLDER}/src" || exit
	pip3 install --no-deps ./
	popd || exit
}

uninstall_igvm_tool()
{
	echo "Uninstalling IGVM tool"

	rm -rf "${IGVM_EXTRACT_FOLDER}"
	pip3 uninstall -y msigvm
}

build_igvm_files()
{
	echo "Reading Kata image dm_verity root hash information from root_hash file"
	ROOT_HASH_FILE="${SCRIPT_DIR}/../root_hash.txt"

	if [[ ! -f "${ROOT_HASH_FILE}" ]]; then
		echo "Could no find image root hash file '${ROOT_HASH_FILE}', aborting"
		exit 1
	fi

	IMAGE_ROOT_HASH=$(sed -e 's/Root hash:\s*//g;t;d' "${ROOT_HASH_FILE}")
	IMAGE_SALT=$(sed -e 's/Salt:\s*//g;t;d' "${ROOT_HASH_FILE}")
	IMAGE_DATA_BLOCKS=$(sed -e 's/Data blocks:\s*//g;t;d' "${ROOT_HASH_FILE}")
	IMAGE_DATA_BLOCK_SIZE=$(sed -e 's/Data block size:\s*//g;t;d' "${ROOT_HASH_FILE}")
	IMAGE_DATA_SECTORS_PER_BLOCK=$((IMAGE_DATA_BLOCK_SIZE / 512))
	IMAGE_DATA_SECTORS=$((IMAGE_DATA_BLOCKS * IMAGE_DATA_SECTORS_PER_BLOCK))
	IMAGE_HASH_BLOCK_SIZE=$(sed -e 's/Hash block size:\s*//g;t;d' "${ROOT_HASH_FILE}")

	# reloading the config file as various variables depend on above values
	load_config_distro

	echo "Building (debug) IGVM files and creating their reference measurement files"
	# we could call into the installed binary '~/.local/bin/igvmgen' when adding to PATH or, better, into 'python3 -m msigvm'
	# however, as we still need the installation directory for the ACPI tables, we leave things as is for now
	# at the same time we seem to need to call pip3 install for invoking the tool at all
	python3 "${IGVM_PY_FILE}" "${IGVM_BUILD_VARS[@]}" -o "${IGVM_FILE_NAME}" -measurement_file "${IGVM_MEASUREMENT_FILE_NAME}" -append "${IGVM_KERNEL_PROD_PARAMS}" -svn "${SVN}"
	python3 "${IGVM_PY_FILE}" "${IGVM_BUILD_VARS[@]}" -o "${IGVM_DBG_FILE_NAME}" -measurement_file "${IGVM_DBG_MEASUREMENT_FILE_NAME}" -append "${IGVM_KERNEL_DEBUG_PARAMS}" -svn "${SVN}"

	out_dir="${OUT_DIR:-.}"
	if [[ "${PWD}" -ef "$(readlink -f "${out_dir}")" ]]; then
		echo "OUT_DIR matches with current dir, not moving build artifacts"
	else
		echo "Moving build artifacts to ${out_dir}"
		mv "${IGVM_FILE_NAME}" "${IGVM_DBG_FILE_NAME}" "${IGVM_MEASUREMENT_FILE_NAME}" "${IGVM_DBG_MEASUREMENT_FILE_NAME}" "${out_dir}"
	fi
}
