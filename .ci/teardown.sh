#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

log_copy_dest="$1"

kata_runtime_log_filename="kata-runtime.log"
kata_runtime_log_path="${log_copy_dest}/${kata_runtime_log_filename}"
kata_runtime_log_prefix="kata-runtime_"

proxy_log_filename="kata-proxy.log"
proxy_log_path="${log_copy_dest}/${proxy_log_filename}"
proxy_log_prefix="kata-proxy_"

shim_log_filename="kata-shim.log"
shim_log_path="${log_copy_dest}/${shim_log_filename}"
shim_log_prefix="kata-shim_"

crio_log_filename="crio.log"
crio_log_path="${log_copy_dest}/${crio_log_filename}"
crio_log_prefix="crio_"

docker_log_filename="docker.log"
docker_log_path="${log_copy_dest}/${docker_log_filename}"
docker_log_prefix="docker_"

collect_data_filename="kata-collect-data.log"
collect_data_log_path="${log_copy_dest}/${collect_data_filename}"
collect_data_log_prefix="kata-collect-data_"

kubelet_log_filename="kubelet.log"
kubelet_log_path="${log_copy_dest}/${kubelet_log_filename}"
kubelet_log_prefix="kubelet_"

collect_script="kata-collect-data.sh"
have_collect_script="no"
[ -n "$(command -v $collect_script)" ] && have_collect_script="yes"

# Copy log files if a destination path is provided, otherwise simply
# display them.
if [ "${log_copy_dest}" ]; then
	# Create the log files
	sudo journalctl --no-pager -t kata-runtime > "${kata_runtime_log_path}"
	sudo journalctl --no-pager -t kata-proxy > "${proxy_log_path}"
	sudo journalctl --no-pager -t kata-shim > "${shim_log_path}"
	sudo journalctl --no-pager -u crio > "${crio_log_path}"
	sudo journalctl --no-pager -u docker > "${docker_log_path}"
	sudo journalctl --no-pager -u kubelet > "${kubelet_log_path}"

	[ "${have_collect_script}" = "yes" ] && sudo -E PATH="$PATH" $collect_script > "${collect_data_log_path}"

	# Split them in 5 MiB subfiles to avoid too large files.
	subfile_size=5242880
	pushd "${log_copy_dest}"
	split -b "${subfile_size}" -d "${kata_runtime_log_path}" "${kata_runtime_log_prefix}"
	split -b "${subfile_size}" -d "${proxy_log_path}" "${proxy_log_prefix}"
	split -b "${subfile_size}" -d "${shim_log_path}" "${shim_log_prefix}"
	split -b "${subfile_size}" -d "${crio_log_path}" "${crio_log_prefix}"
	split -b "${subfile_size}" -d "${docker_log_path}" "${docker_log_prefix}"
	split -b "${subfile_size}" -d "${kubelet_log_path}" "${kubelet_log_prefix}"

	[ "${have_collect_script}" = "yes" ] &&  split -b "${subfile_size}" -d "${collect_data_log_path}" "${collect_data_log_prefix}"

	prefixes=""
	prefixes+=" ${kata_runtime_log_prefix}"
	prefixes+=" ${proxy_log_prefix}"
	prefixes+=" ${shim_log_prefix}"
	prefixes+=" ${crio_log_prefix}"
	prefixes+=" ${docker_log_prefix}"
	prefixes+=" ${kubelet_log_prefix}"

	 [ "${have_collect_script}" = "yes" ] && prefixes+=" ${collect_data_log_prefix}"

	for prefix in $prefixes
	do
		gzip -9 "$prefix"*
	done

	popd
else
	echo "Kata Containers Runtime Log:"
	sudo journalctl --no-pager -t kata-runtime

	echo "Kata Containers Proxy Log:"
	sudo journalctl --no-pager -t kata-proxy

	echo "Kata Containers Shim Log:"
	sudo journalctl --no-pager -t kata-shim

	echo "CRI-O Log:"
	sudo journalctl --no-pager -u crio

	echo "Docker Log:"
	sudo journalctl --no-pager -u docker

	echo "Kubelet Log:"
	sudo journalctl --no-pager -u kubelet

	if [ "${have_collect_script}" = "yes" ]
	then
		echo "Kata Collect Data script output"
		sudo -E PATH="$PATH" $collect_script
	fi
fi
