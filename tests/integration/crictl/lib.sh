#!/bin/bash
#
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

testContainerStart() {
	# no_container_yaml set to 1 will not create container_yaml
	# because caller has created its own container_yaml.
	no_container_yaml=${1:-0}

	local pod_yaml=${REPORT_DIR}/pod.yaml
	local container_yaml=${REPORT_DIR}/container.yaml
	local image="busybox:latest"

	cat << EOF > "${pod_yaml}"
metadata:
  name: busybox-sandbox1
  namespace: default
  uid: busybox-sandbox1-uid
EOF

	#TestContainerSwap has created its own container_yaml.
	if [ $no_container_yaml -ne 1 ]; then
		cat << EOF > "${container_yaml}"
metadata:
  name: busybox-killed-vmm
  namespace: default
  uid: busybox-killed-vmm-uid
image:
  image: "$image"
command:
- top
EOF
	fi

	pause_image=$(sudo crictl info | jq -r .config.sandboxImage)
	sudo crictl pull "$pause_image"
	sudo crictl pull $image
	podid=$(sudo crictl runp $pod_yaml)
	cid=$(sudo crictl create $podid $container_yaml $pod_yaml)
	sudo crictl start $cid
}

function testContainerStop() {
	info "show pod $podid"
	sudo crictl --timeout=20s pods --id $podid
	info "stop pod $podid"
	sudo crictl --timeout=20s stopp $podid
	info "remove pod $podid"
	sudo crictl --timeout=20s rmp $podid
}
