#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Two-node end-to-end test of storage-capacity-aware scheduling with
# csi-kata-directvolume and Kata Containers.
#
# One capacity pool gets 2Gi on node A and 8Gi on node B. The test checks
# that the capacity is published, that the scheduler places a 4Gi volume on
# node B, that a 5Gi volume stays pending while the 4Gi one exists, that it
# gets provisioned on node B once the first volume is deleted, and that a
# restart of the driver keeps every reservation.
#
# By default the script creates a kind cluster with two worker nodes and
# installs Kata Containers with kata-deploy, which needs /dev/kvm on the host.
# Set USE_EXISTING_CLUSTER=true, NODE_A and NODE_B to use an existing cluster
# instead; Kata Containers must then already be installed.
#
# Usage: run-capacity-e2e.sh [all|setup|test|report|teardown]

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
driver_dir=$(cd "${script_dir}/../.." && pwd)

: "${KIND_CLUSTER_NAME:=csi-capacity-e2e}"
: "${KIND_NODE_IMAGE:=}"
: "${USE_EXISTING_CLUSTER:=false}"
: "${KEEP_CLUSTER:=false}"
: "${KATA_VERSION:=4.3.1}"
: "${KATA_SHIM:=qemu-runtime-rs}"
: "${KATA_DEPLOY_CHART:=oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy}"
: "${CSI_IMAGE_REGISTRY:=localhost}"
: "${CSI_IMAGE_TAG:=e2e}"
: "${BUILD_IMAGE:=true}"
: "${CAPACITY_POOL:=e2e}"
: "${NODE_A_CAPACITY:=2Gi}"
: "${NODE_B_CAPACITY:=8Gi}"
: "${FIRST_VOLUME_SIZE:=4Gi}"
: "${SECOND_VOLUME_SIZE:=5Gi}"
: "${POLL_INTERVAL_SECONDS:=10}"
: "${PUBLICATION_TIMEOUT_SECONDS:=30}"
: "${RECOVERY_TIMEOUT_SECONDS:=30}"
: "${POD_TIMEOUT:=600s}"
: "${ARTIFACTS:=${PWD}/csi-capacity-e2e-artifacts}"
# Host directories which back the storage pools of the kind nodes.
: "${NODE_STORAGE_DIR:=/var/tmp/${KIND_CLUSTER_NAME}-storage}"
: "${NODE_A:=}"
: "${NODE_B:=}"
# The image of the test pods, from versions.yaml unless it is set.
if [[ -z "${TEST_IMAGE:-}" ]]; then
	versions_yaml="${driver_dir}/../../../versions.yaml"
	TEST_IMAGE="$(yq '.docker_images.busybox.registry' "${versions_yaml}")@$(yq '.docker_images.busybox.digest' "${versions_yaml}")"
fi

readonly driver_name="directvolume.csi.katacontainers.io"
readonly driver_namespace="kata-directvolume"
readonly daemonset="csi-kata-directvol-plugin"
readonly capacity_configmap="csi-kata-directvol-capacity"
readonly topology_key="topology.directvolume.csi/node"
readonly storage_class="csi-capacity-e2e"
readonly test_namespace="csi-capacity-e2e"
readonly storage_pool_path="/tmp/kata-directvol-storages"
readonly csi_image="${CSI_IMAGE_REGISTRY}/kata-directvolume:${CSI_IMAGE_TAG}"
runtime_class="kata-${KATA_SHIM}"
readonly runtime_class

timeline="${ARTIFACTS}/capacity-timeline.tsv"
watch_log="${ARTIFACTS}/capacity-watch.tsv"
results="${ARTIFACTS}/results.tsv"
watch_pid=""

info() {
	echo "[$(date -u +%H:%M:%S)] $*" >&2
}

die() {
	echo "[$(date -u +%H:%M:%S)] ERROR: $*" >&2
	exit 1
}

# pass records a verified expectation for the report.
pass() {
	info "PASS: $*"
	printf 'PASS\t%s\n' "$*" >>"${results}"
}

fail() {
	printf 'FAIL\t%s\n' "$*" >>"${results}"
	die "$*"
}

now() {
	date +%s.%N
}

elapsed_since() {
	awk -v start="$1" -v end="$(now)" 'BEGIN { printf "%.1f", end - start }'
}

# at_most succeeds if the number of seconds $1 does not exceed $2.
at_most() {
	awk -v value="$1" -v limit="$2" 'BEGIN { exit !(value <= limit) }'
}

# klog_time converts the timestamp of a klog line to seconds since the epoch.
klog_time() {
	[[ "$1" =~ ^[IWEF]([0-9]{2})([0-9]{2})\ ([0-9:.]+) ]] || return 1
	date -u -d "$(date -u +%Y)-${BASH_REMATCH[1]}-${BASH_REMATCH[2]} ${BASH_REMATCH[3]}" +%s.%N
}

# source_revision names the commit of the driver sources, with "-dirty" if
# they have uncommitted changes.
source_revision() {
	local revision
	revision=$(git -C "${driver_dir}" rev-parse --short HEAD 2>/dev/null) || {
		echo unknown
		return
	}
	git -C "${driver_dir}" diff --quiet HEAD -- . 2>/dev/null || revision="${revision}-dirty"
	echo "${revision}"
}

# to_bytes converts a Kubernetes quantity without fraction to bytes.
to_bytes() {
	local quantity="$1"
	[[ "${quantity}" =~ ^([0-9]+)([A-Za-z]*)$ ]] || die "cannot convert quantity '${quantity}'"
	local number="${BASH_REMATCH[1]}"
	case "${BASH_REMATCH[2]}" in
	"") echo "${number}" ;;
	Ki) echo $((number * 1024)) ;;
	Mi) echo $((number * 1024 * 1024)) ;;
	Gi) echo $((number * 1024 * 1024 * 1024)) ;;
	Ti) echo $((number * 1024 * 1024 * 1024 * 1024)) ;;
	k) echo $((number * 1000)) ;;
	M) echo $((number * 1000000)) ;;
	G) echo $((number * 1000000000)) ;;
	T) echo $((number * 1000000000000)) ;;
	*) die "unsupported unit in quantity '${quantity}'" ;;
	esac
}

# wait_until runs a command until it succeeds or the timeout expires.
wait_until() {
	local timeout="$1" what="$2"
	shift 2
	local deadline=$(($(date +%s) + timeout))
	until "$@"; do
		if [[ "$(date +%s)" -ge "${deadline}" ]]; then
			return 1
		fi
		sleep 1
	done
	info "${what}"
}

################################################################################
# Cluster and component setup
################################################################################

create_cluster() {
	if kind get clusters 2>/dev/null | grep -qx "${KIND_CLUSTER_NAME}"; then
		info "kind cluster ${KIND_CLUSTER_NAME} exists already"
		return
	fi
	[[ -c /dev/kvm ]] || die "/dev/kvm is missing, Kata Containers needs hardware virtualization"

	# Kata needs these modules. kata-deploy cannot load them inside the
	# kind nodes, but it uses them when the host has them loaded.
	local module
	for module in kvm vhost vhost_vsock vhost_net tun; do
		[[ -d "/sys/module/${module}" ]] || sudo modprobe "${module}" || info "cannot load ${module}"
	done

	# Back the storage pool of each node with its own host directory: /tmp
	# is a tmpfs in kind nodes.
	local storage_root="${NODE_STORAGE_DIR}"
	mkdir -p "${storage_root}/worker" "${storage_root}/worker2"

	local config="${ARTIFACTS}/kind-config.yaml"
	cat >"${config}" <<EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
- role: worker
  extraMounts:
  - hostPath: ${storage_root}/worker
    containerPath: ${storage_pool_path}
- role: worker
  extraMounts:
  - hostPath: ${storage_root}/worker2
    containerPath: ${storage_pool_path}
EOF
	local image_args=()
	[[ -n "${KIND_NODE_IMAGE}" ]] && image_args=(--image "${KIND_NODE_IMAGE}")
	info "creating kind cluster ${KIND_CLUSTER_NAME}"
	kind create cluster --name "${KIND_CLUSTER_NAME}" --config "${config}" "${image_args[@]}" --wait 5m
}

# prepare_kind_nodes adapts the kind worker nodes to Kata Containers:
# runtime-rs manages the systemd cgroup of a sandbox through the system
# D-Bus, which kind nodes do not run, and QEMU backs the guest memory with a
# file in /dev/shm, which Docker limits to 64 MiB. The driver also mounts
# /var/lib/spdk/vhost, which nodes without SPDK do not have.
prepare_kind_nodes() {
	local node
	for node in "${NODE_A}" "${NODE_B}"; do
		info "preparing kind node ${node} for Kata Containers"
		docker exec "${node}" sh -c '
			set -e
			if [ ! -S /run/dbus/system_bus_socket ]; then
				if ! command -v dbus-daemon >/dev/null; then
					apt-get update -qq
					DEBIAN_FRONTEND=noninteractive apt-get install -y -qq dbus >/dev/null
				fi
				systemctl start dbus.socket dbus.service
			fi
			mount -o remount,size=8g /dev/shm
			mkdir -p /var/lib/spdk/vhost
		'
	done
}

select_nodes() {
	if [[ -z "${NODE_A}" || -z "${NODE_B}" ]]; then
		[[ "${USE_EXISTING_CLUSTER}" == "true" ]] && die "set NODE_A and NODE_B when USE_EXISTING_CLUSTER=true"
		NODE_A="${KIND_CLUSTER_NAME}-worker"
		NODE_B="${KIND_CLUSTER_NAME}-worker2"
	fi
	kubectl get node "${NODE_A}" "${NODE_B}" >/dev/null || die "nodes ${NODE_A} and ${NODE_B} must exist"
	info "node A (${NODE_A_CAPACITY}): ${NODE_A}, node B (${NODE_B_CAPACITY}): ${NODE_B}"
}

# load_image copies an image from the local Docker into the kind nodes, so
# that they need not pull it. Images which Docker does not have are pulled by
# the nodes as usual, like images with a digest, which kind would import
# without their name. The second argument optionally selects the nodes.
load_image() {
	local image="$1" nodes="${2:-}"
	[[ "${USE_EXISTING_CLUSTER}" == "true" || "${image}" == *@* ]] && return
	docker image inspect "${image}" >/dev/null 2>&1 || return 0

	local node_args=()
	[[ -n "${nodes}" ]] && node_args=(--nodes "${nodes}")
	# Save a single platform: with the containerd image store, saving a
	# multi-platform image refers to manifests which are not available.
	local archive platform
	archive=$(mktemp -t kind-image.XXXXXX.tar)
	platform=$(docker version -f '{{.Server.Os}}/{{.Server.Arch}}')
	if docker save --platform "${platform}" -o "${archive}" "${image}" 2>/dev/null; then
		kind load image-archive --name "${KIND_CLUSTER_NAME}" "${node_args[@]}" "${archive}" >/dev/null
	else
		kind load docker-image --name "${KIND_CLUSTER_NAME}" "${node_args[@]}" "${image}" >/dev/null
	fi
	rm -f "${archive}"
}

install_kata() {
	if kubectl get runtimeclass "${runtime_class}" >/dev/null 2>&1; then
		info "runtime class ${runtime_class} exists already"
	else
		[[ "${USE_EXISTING_CLUSTER}" == "true" ]] && die "runtime class ${runtime_class} is missing"
		load_image "quay.io/kata-containers/kata-deploy:${KATA_VERSION}" "${NODE_A},${NODE_B}"
		info "installing Kata Containers ${KATA_VERSION} (${KATA_SHIM}) with kata-deploy"
		helm upgrade --install kata-deploy "${KATA_DEPLOY_CHART}" \
			--version "${KATA_VERSION}" \
			--namespace kube-system \
			--set imagePullPolicy=IfNotPresent \
			--set shims.disableAll=true \
			--set "shims.${KATA_SHIM}.enabled=true" \
			--set "defaultShim.amd64=${KATA_SHIM}" \
			--wait --timeout 20m
		wait_until 300 "runtime class ${runtime_class} is available" \
			kubectl get runtimeclass "${runtime_class}"
	fi

	# Prove that pods run in a Kata VM: the guest kernel is not the host one.
	load_image "${TEST_IMAGE}"
	kubectl create namespace "${test_namespace}" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
	kubectl apply -n "${test_namespace}" -f - <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: kata-smoke
spec:
  runtimeClassName: ${runtime_class}
  nodeName: ${NODE_B}
  restartPolicy: Never
  containers:
  - name: smoke
    image: ${TEST_IMAGE}
    command: ["uname", "-r"]
EOF
	kubectl wait -n "${test_namespace}" --for=jsonpath='{.status.phase}'=Succeeded pod/kata-smoke --timeout="${POD_TIMEOUT}"
	local guest_kernel host_kernel
	guest_kernel=$(kubectl logs -n "${test_namespace}" kata-smoke)
	host_kernel=$(uname -r)
	kubectl delete -n "${test_namespace}" pod kata-smoke --wait=false >/dev/null
	[[ "${guest_kernel}" != "${host_kernel}" ]] || die "kata-smoke ran with the host kernel ${host_kernel}"
	echo "${guest_kernel}" >"${ARTIFACTS}/kata-guest-kernel.txt"
	pass "pods with runtime class ${runtime_class} run in a Kata VM (guest kernel ${guest_kernel}, host kernel ${host_kernel})"
}

build_driver_image() {
	if [[ "${BUILD_IMAGE}" == "true" ]]; then
		info "building ${csi_image}"
		local build_args=() proxy_var
		for proxy_var in http_proxy https_proxy no_proxy HTTP_PROXY HTTPS_PROXY NO_PROXY; do
			[[ -n "${!proxy_var:-}" ]] && build_args+=(--build-arg "${proxy_var}=${!proxy_var}")
		done
		(
			cd "${driver_dir}"
			mkdir -p bin
			CGO_ENABLED=0 GOOS=linux go build -ldflags "-X main.version=e2e-$(source_revision)" \
				-o bin/directvolplugin ./cmd/directvolplugin
			docker build -q "${build_args[@]}" -t "${csi_image}" . >/dev/null
		)
	fi
	load_image "${csi_image}"
	local image
	grep -E '^\s*image:' "${driver_dir}/deploy/kata-directvolume/csi-directvol-plugin.yaml" | awk '{print $2}' |
		while read -r image; do
			[[ "${image}" == */kata-directvolume:* ]] && continue
			load_image "${image}"
		done
}

deploy_driver() {
	info "deploying csi-kata-directvolume with the capacity pool '${CAPACITY_POOL}'"
	"${driver_dir}/deploy/rbac-deploy.sh" >"${ARTIFACTS}/rbac-deploy.log" 2>&1

	# The per-node capacity configuration: one key per node.
	kubectl create configmap "${capacity_configmap}" -n "${driver_namespace}" \
		--from-literal="${NODE_A}=${CAPACITY_POOL}=${NODE_A_CAPACITY}" \
		--from-literal="${NODE_B}=${CAPACITY_POOL}=${NODE_B_CAPACITY}" \
		--dry-run=client -o yaml | kubectl apply -f - >/dev/null

	CAPACITY_POLL_INTERVAL="${POLL_INTERVAL_SECONDS}s" \
		KATA_DIRECTVOLUME_REGISTRY="${CSI_IMAGE_REGISTRY}" \
		KATA_DIRECTVOLUME_TAG="${CSI_IMAGE_TAG}" \
		"${driver_dir}/deploy/directvol-deploy.sh" >"${ARTIFACTS}/directvol-deploy.log" 2>&1 ||
		{
			cat "${ARTIFACTS}/directvol-deploy.log" >&2
			die "directvol-deploy.sh failed"
		}
	kubectl rollout status -n "${driver_namespace}" "daemonset/${daemonset}" --timeout=300s
}

setup() {
	mkdir -p "${ARTIFACTS}"
	if [[ "${USE_EXISTING_CLUSTER}" != "true" ]]; then
		create_cluster
	fi
	select_nodes
	[[ "${USE_EXISTING_CLUSTER}" == "true" ]] || prepare_kind_nodes
	install_kata
	build_driver_image
	deploy_driver
}

################################################################################
# Helpers for the test
################################################################################

driver_pod() {
	kubectl get pods -n "${driver_namespace}" -l app.kubernetes.io/name="${daemonset}" \
		--field-selector spec.nodeName="$1" -o jsonpath='{.items[0].metadata.name}'
}

# capacity_objects prints the CSIStorageCapacity objects of the test
# StorageClass for a node as a JSON array.
capacity_objects() {
	kubectl get csistoragecapacities -n "${driver_namespace}" -o json |
		jq -c --arg sc "${storage_class}" --arg key "${topology_key}" --arg node "$1" \
			'[.items[] | select(.storageClassName == $sc and .nodeTopology.matchLabels[$key] == $node)]'
}

# published_capacity prints "<capacity> <maximumVolumeSize>" in bytes for a
# node, after checking the invariants of the published object.
published_capacity() {
	local node="$1" objects count
	objects=$(capacity_objects "${node}")
	count=$(jq length <<<"${objects}")
	[[ "${count}" -eq 1 ]] || {
		echo "node ${node} has ${count} CSIStorageCapacity objects for ${storage_class}" >&2
		return 1
	}
	local capacity maximum
	capacity=$(to_bytes "$(jq -r '.[0].capacity // "0"' <<<"${objects}")")
	maximum=$(jq -r '.[0].maximumVolumeSize // ""' <<<"${objects}")
	if [[ -n "${maximum}" ]]; then
		maximum=$(to_bytes "${maximum}")
		[[ "${maximum}" -le "${capacity}" ]] || fail "node ${node}: maximumVolumeSize ${maximum} exceeds capacity ${capacity}"
	else
		maximum="-"
	fi
	[[ "${capacity}" -ge 0 ]] || fail "node ${node}: negative capacity ${capacity}"
	echo "${capacity} ${maximum}"
}

capacity_is() {
	local node="$1" expected="$2" actual
	actual=$(published_capacity "${node}" 2>/dev/null) || return 1
	[[ "${actual%% *}" -eq "${expected}" ]]
}

capacities_are() {
	capacity_is "${NODE_A}" "$1" && capacity_is "${NODE_B}" "$2"
}

pod_unschedulable() {
	[[ "$(kubectl get pod -n "${test_namespace}" "$1" -o jsonpath='{.status.conditions[?(@.type=="PodScheduled")].reason}')" == "Unschedulable" ]]
}

pv_deleted() {
	! kubectl get pv "$1" >/dev/null 2>&1
}

record_capacity() {
	local phase="$1" node values
	for node in "${NODE_A}" "${NODE_B}"; do
		values=$(published_capacity "${node}") || fail "${phase}: no capacity published for ${node}"
		printf '%s\t%s\t%s\t%s\n' "${phase}" "${node}" "${values% *}" "${values#* }" >>"${timeline}"
	done
	snapshot "${phase}"
}

# snapshot saves the PVCs, pods and persistent volumes of the test.
snapshot() {
	local file="${ARTIFACTS}/snapshots/$1.txt"
	mkdir -p "${ARTIFACTS}/snapshots"
	{
		kubectl get pvc,pod -n "${test_namespace}" -o wide 2>&1
		echo
		kubectl get pv -o custom-columns='PV:.metadata.name,CLAIM:.spec.claimRef.name,CAPACITY:.spec.capacity.storage,STORAGECLASS:.spec.storageClassName,NODE-AFFINITY:.spec.nodeAffinity.required.nodeSelectorTerms[*].matchExpressions[*].values' 2>&1
	} >"${file}"
}

# start_capacity_watch records every change of a CSIStorageCapacity object,
# with the time at which it was seen, as whitespace separated
# <time> <event> <storage class> <node> <capacity> <maximumVolumeSize>.
start_capacity_watch() {
	: >"${watch_log}"
	# A session of its own, so that stop_capacity_watch can stop the pipeline.
	# shellcheck disable=SC2016 # The inner shell expands its arguments.
	setsid bash -c '
		kubectl get csistoragecapacities -n "$1" --watch --output-watch-events \
			-o jsonpath="{.type}{\"\t\"}{.object.storageClassName}{\"\t\"}{.object.nodeTopology.matchLabels.$2}{\"\t\"}{.object.capacity}{\"\t\"}{.object.maximumVolumeSize}{\"\n\"}" |
			while IFS= read -r line; do printf "%s\t%s\n" "$(date +%s.%N)" "${line}"; done
	' capacity-watch "${driver_namespace}" "${topology_key//./\\.}" >>"${watch_log}" 2>/dev/null &
	watch_pid=$!
}

stop_capacity_watch() {
	[[ -n "${watch_pid}" ]] || return 0
	kill -- "-${watch_pid}" 2>/dev/null || true
	watch_pid=""
}

# first_published prints "<time> <capacity> <maximumVolumeSize>" of the
# first capacity of a node seen after a point in time which is at least the
# given number of bytes.
first_published() {
	local node="$1" after="$2" needed="$3"
	local time event sc object_node capacity maximum
	# kubectl aligns the columns with spaces.
	while read -r time event sc object_node capacity maximum; do
		[[ "${sc}" == "${storage_class}" && "${object_node}" == "${node}" && "${event}" != "DELETED" ]] || continue
		at_most "${after}" "${time}" || continue
		if [[ "$(to_bytes "${capacity}")" -ge "${needed}" ]]; then
			echo "${time} $(to_bytes "${capacity}") $(to_bytes "${maximum:-0}")"
			return 0
		fi
	done <"${watch_log}"
	return 1
}

no_test_volumes() {
	[[ -z "$(kubectl get pv -o jsonpath="{.items[?(@.spec.storageClassName==\"${storage_class}\")].metadata.name}")" ]]
}

# reset_test_state removes what an earlier run of the test left behind.
reset_test_state() {
	if kubectl get namespace "${test_namespace}" >/dev/null 2>&1; then
		kubectl delete pod,pvc --all -n "${test_namespace}" --wait=true --timeout=300s >/dev/null
	fi
	wait_until 180 "no volumes of an earlier run are left" no_test_volumes ||
		die "the volumes of an earlier run were not deleted"
	kubectl delete storageclass "${storage_class}" --ignore-not-found >/dev/null
}

require_capacity() {
	local phase="$1" node="$2" expected="$3" actual
	actual=$(published_capacity "${node}") || fail "${phase}: no capacity published for ${node}"
	[[ "${actual%% *}" -eq "${expected}" ]] ||
		fail "${phase}: ${node} publishes ${actual%% *} bytes, expected ${expected}"
}

create_workload() {
	local name="$1" size="$2"
	kubectl apply -n "${test_namespace}" -f - >/dev/null <<EOF
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ${name}
spec:
  accessModes: [ReadWriteOnce]
  storageClassName: ${storage_class}
  resources:
    requests:
      storage: ${size}
---
apiVersion: v1
kind: Pod
metadata:
  name: ${name}
spec:
  runtimeClassName: ${runtime_class}
  containers:
  - name: app
    image: ${TEST_IMAGE}
    command: ["sleep", "infinity"]
    volumeMounts:
    - name: data
      mountPath: /data
  volumes:
  - name: data
    persistentVolumeClaim:
      claimName: ${name}
EOF
}

pv_of() {
	kubectl get pvc -n "${test_namespace}" "$1" -o jsonpath='{.spec.volumeName}'
}

# driver_logs saves the logs of the driver and provisioner of a node.
driver_logs() {
	local node="$1" suffix="$2" pod
	pod=$(driver_pod "${node}")
	kubectl logs -n "${driver_namespace}" "${pod}" -c kata-directvolume >"${ARTIFACTS}/logs/driver-${node}-${suffix}.log" 2>&1 || true
	kubectl logs -n "${driver_namespace}" "${pod}" -c csi-provisioner >"${ARTIFACTS}/logs/provisioner-${node}-${suffix}.log" 2>&1 || true
}

create_volume_calls() {
	local node="$1" pv_name="$2" pod
	pod=$(driver_pod "${node}")
	kubectl logs -n "${driver_namespace}" "${pod}" -c kata-directvolume 2>/dev/null |
		grep -c "createVolume with request: name:\"${pv_name}\"" || true
}

backing_files() {
	if [[ "${USE_EXISTING_CLUSTER}" == "true" ]]; then
		kubectl exec -n "${driver_namespace}" "$(driver_pod "$1")" -c kata-directvolume -- \
			find "${storage_pool_path}" -name 'directvol-rawdisk.*' 2>/dev/null | sort
	else
		docker exec "$1" find "${storage_pool_path}" -name 'directvol-rawdisk.*' | sort
	fi
}

################################################################################
# Test phases
################################################################################

test_publication() {
	local start="$1"
	local expected_a expected_b
	expected_a=$(to_bytes "${NODE_A_CAPACITY}")
	expected_b=$(to_bytes "${NODE_B_CAPACITY}")

	kubectl apply -f - >/dev/null <<EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: ${storage_class}
provisioner: ${driver_name}
parameters:
  katacontainers.direct.volume/volumetype: directvol
  katacontainers.direct.volume/fstype: ext4
  katacontainers.direct.volume/capacity-pool: ${CAPACITY_POOL}
reclaimPolicy: Delete
volumeBindingMode: WaitForFirstConsumer
EOF

	[[ "$(kubectl get csidriver "${driver_name}" -o jsonpath='{.spec.storageCapacity}')" == "true" ]] ||
		fail "CSIDriver ${driver_name} does not enable storageCapacity"
	pass "CSIDriver ${driver_name} has storageCapacity: true"

	wait_until "${PUBLICATION_TIMEOUT_SECONDS}" "capacity published" capacities_are "${expected_a}" "${expected_b}" ||
		fail "the capacity of ${NODE_A} and ${NODE_B} was not published within ${PUBLICATION_TIMEOUT_SECONDS}s of the deployment"
	local published_after
	published_after=$(elapsed_since "${start}")
	echo "publication_seconds ${published_after}" >>"${ARTIFACTS}/timings.txt"
	record_capacity "1-before-allocation"
	pass "CSIStorageCapacity of ${storage_class}: ${NODE_A}=${NODE_A_CAPACITY}, ${NODE_B}=${NODE_B_CAPACITY}, within ${published_after}s of the deployment"

	local node owner
	for node in "${NODE_A}" "${NODE_B}"; do
		owner=$(capacity_objects "${node}" | jq -r '.[0].metadata.ownerReferences[0] | "\(.kind)/\(.name)"')
		[[ "${owner}" == "DaemonSet/${daemonset}" ]] || fail "CSIStorageCapacity of ${node} is owned by ${owner}"
	done
	pass "the CSIStorageCapacity objects are owned by DaemonSet/${daemonset}"
}

test_rbac() {
	local sa="system:serviceaccount:${driver_namespace}:csi-provisioner" verb
	for verb in get list watch create update patch delete; do
		kubectl auth can-i --as="${sa}" -n "${driver_namespace}" "${verb}" csistoragecapacities.storage.k8s.io >/dev/null ||
			fail "${sa} cannot ${verb} csistoragecapacities"
	done
	kubectl auth can-i --as="${sa}" -n "${driver_namespace}" get pods >/dev/null ||
		fail "${sa} cannot get its pod, which --capacity-ownerref-level needs"
	for verb in get list watch; do
		kubectl auth can-i --as="${sa}" "${verb}" storageclasses.storage.k8s.io >/dev/null ||
			fail "${sa} cannot ${verb} storageclasses"
		kubectl auth can-i --as="${sa}" "${verb}" csinodes.storage.k8s.io >/dev/null ||
			fail "${sa} cannot ${verb} csinodes"
	done
	pass "the csi-provisioner service account may manage CSIStorageCapacity objects and look up its owner"
}

test_scheduling() {
	info "creating a ${FIRST_VOLUME_SIZE} volume, which only fits on ${NODE_B}"
	create_workload first "${FIRST_VOLUME_SIZE}"
	kubectl wait -n "${test_namespace}" --for=condition=Ready pod/first --timeout="${POD_TIMEOUT}" ||
		fail "pod first did not become ready"

	local node pv affinity
	node=$(kubectl get pod -n "${test_namespace}" first -o jsonpath='{.spec.nodeName}')
	[[ "${node}" == "${NODE_B}" ]] || fail "pod first runs on ${node}, expected ${NODE_B}"
	[[ "$(kubectl get pvc -n "${test_namespace}" first -o jsonpath='{.status.phase}')" == "Bound" ]] ||
		fail "PVC first is not bound"
	pv=$(pv_of first)
	affinity=$(kubectl get pv "${pv}" -o json |
		jq -r --arg key "${topology_key}" '[.spec.nodeAffinity.required.nodeSelectorTerms[].matchExpressions[] | select(.key == $key) | .values[]] | join(",")')
	[[ "${affinity}" == "${NODE_B}" ]] || fail "PV ${pv} has node affinity '${affinity}', expected ${NODE_B}"
	pass "PVC first is Bound, pod first is Running on ${NODE_B}, PV ${pv} has node affinity ${topology_key}=${NODE_B}"

	[[ "$(create_volume_calls "${NODE_A}" "${pv}")" -eq 0 ]] || fail "CreateVolume for ${pv} was called on ${NODE_A}"
	[[ "$(create_volume_calls "${NODE_B}" "${pv}")" -eq 1 ]] || fail "CreateVolume for ${pv} was not called exactly once on ${NODE_B}"
	local failures
	failures=$(kubectl get events -n "${test_namespace}" --field-selector involvedObject.name=first -o json |
		jq '[.items[] | select(.reason == "ProvisioningFailed" or .reason == "FailedScheduling")] | length')
	[[ "${failures}" -eq 0 ]] || fail "first needed ${failures} failed scheduling or provisioning attempts"
	pass "no CreateVolume on ${NODE_A}, one CreateVolume on ${NODE_B}, no failed scheduling or provisioning attempt"

	kubectl exec -n "${test_namespace}" first -- sh -c 'echo first > /data/marker && sync && df -k /data' \
		>"${ARTIFACTS}/first-df.txt" || fail "the volume of pod first is not writable"
	pass "the volume of pod first is writable"

	local reduced
	reduced=$(($(to_bytes "${NODE_B_CAPACITY}") - $(to_bytes "${FIRST_VOLUME_SIZE}")))
	wait_until "${PUBLICATION_TIMEOUT_SECONDS}" "capacity of ${NODE_B} reduced" capacity_is "${NODE_B}" "${reduced}" ||
		fail "${NODE_B} does not publish ${reduced} bytes after the allocation"
	require_capacity "after allocation" "${NODE_A}" "$(to_bytes "${NODE_A_CAPACITY}")"
	record_capacity "2-after-allocation"
	pass "${NODE_B} publishes exactly ${FIRST_VOLUME_SIZE} less after the allocation"
}

test_exhaustion() {
	info "creating a ${SECOND_VOLUME_SIZE} volume, which fits on no node"
	local files_a files_b
	files_a=$(backing_files "${NODE_A}")
	files_b=$(backing_files "${NODE_B}")
	create_workload second "${SECOND_VOLUME_SIZE}"

	wait_until 60 "pod second is unschedulable" pod_unschedulable second ||
		fail "pod second was not reported as unschedulable"
	# Give the scheduler a few more attempts to place it.
	sleep "$((2 * POLL_INTERVAL_SECONDS))"

	[[ "$(kubectl get pod -n "${test_namespace}" second -o jsonpath='{.status.phase}')" == "Pending" ]] ||
		fail "pod second is not pending"
	[[ "$(kubectl get pvc -n "${test_namespace}" second -o jsonpath='{.status.phase}')" == "Pending" ]] ||
		fail "PVC second is not pending"
	[[ -z "$(pv_of second)" ]] || fail "PVC second was provisioned"
	[[ -z "$(kubectl get pvc -n "${test_namespace}" second -o jsonpath='{.metadata.annotations.volume\.kubernetes\.io/selected-node}')" ]] ||
		fail "the scheduler selected a node for PVC second"
	local reason
	reason=$(kubectl get events -n "${test_namespace}" --field-selector involvedObject.name=second,reason=FailedScheduling -o json |
		jq -r '.items | length')
	[[ "${reason}" -gt 0 ]] || fail "no FailedScheduling event for pod second"
	kubectl get events -n "${test_namespace}" --field-selector involvedObject.name=second -o yaml >"${ARTIFACTS}/second-pending-events.yaml"
	pass "pod second stays Pending (FailedScheduling) and PVC second stays unprovisioned"

	local logs node
	for node in "${NODE_A}" "${NODE_B}"; do
		logs=$(kubectl logs -n "${driver_namespace}" "$(driver_pod "${node}")" -c kata-directvolume)
		grep -q "createVolume with request: name:\"pvc-$(kubectl get pvc -n "${test_namespace}" second -o jsonpath='{.metadata.uid}')\"" <<<"${logs}" &&
			fail "CreateVolume was called on ${node} for PVC second"
	done
	[[ "$(backing_files "${NODE_A}")" == "${files_a}" && "$(backing_files "${NODE_B}")" == "${files_b}" ]] ||
		fail "backend storage was created while PVC second is pending"
	record_capacity "3-while-exhausted"
	pass "no CreateVolume and no backend storage for PVC second on either node, no negative capacity"
}

test_recovery() {
	info "deleting the first workload"
	local pv handle
	pv=$(pv_of first)
	handle=$(kubectl get pv "${pv}" -o jsonpath='{.spec.csi.volumeHandle}')
	kubectl delete -n "${test_namespace}" pod first --wait=true --timeout=300s >/dev/null
	kubectl delete -n "${test_namespace}" pvc first --wait=true --timeout=120s >/dev/null
	wait_until 180 "PV ${pv} deleted" pv_deleted "${pv}" ||
		fail "PV ${pv} was not deleted"

	# Measure from the successful DeleteVolume, as logged by the driver.
	local logs line deleted
	logs=$(kubectl logs -n "${driver_namespace}" "$(driver_pod "${NODE_B}")" -c kata-directvolume)
	line=$(grep -m 1 "volume ${handle} successfully deleted" <<<"${logs}") ||
		fail "no successful DeleteVolume of ${handle} on ${NODE_B}"
	deleted=$(klog_time "${line}") || fail "cannot read the time of '${line}'"

	kubectl wait -n "${test_namespace}" --for=condition=Ready pod/second --timeout="${POD_TIMEOUT}" ||
		fail "pod second did not become ready"
	local ready_after node
	ready_after=$(elapsed_since "${deleted}")
	node=$(kubectl get pod -n "${test_namespace}" second -o jsonpath='{.spec.nodeName}')
	[[ "${node}" == "${NODE_B}" ]] || fail "pod second runs on ${node}, expected ${NODE_B}"
	[[ "$(kubectl get pvc -n "${test_namespace}" second -o jsonpath='{.status.phase}')" == "Bound" ]] ||
		fail "PVC second is not bound"

	# The pending pod takes the space as soon as it is published, so look
	# at the recorded changes instead of the current value.
	local needed published time capacity maximum recovery
	needed=$(to_bytes "${SECOND_VOLUME_SIZE}")
	published=$(first_published "${NODE_B}" "${deleted}" "${needed}") ||
		fail "${NODE_B} did not publish ${SECOND_VOLUME_SIZE} after DeleteVolume of ${pv}"
	read -r time capacity maximum <<<"${published}"
	recovery=$(awk -v a="${time}" -v b="${deleted}" 'BEGIN { printf "%.2f", a - b }')
	at_most "${recovery}" "${RECOVERY_TIMEOUT_SECONDS}" ||
		fail "${NODE_B} published enough capacity ${recovery}s after DeleteVolume, the limit is ${RECOVERY_TIMEOUT_SECONDS}s"
	[[ "${capacity}" -eq "$(to_bytes "${NODE_B_CAPACITY}")" ]] ||
		fail "${NODE_B} published ${capacity} bytes after the deletion, expected ${NODE_B_CAPACITY}"
	printf '%s\t%s\t%s\t%s\n' "4-after-deletion" "${NODE_B}" "${capacity}" "${maximum}" >>"${timeline}"
	snapshot "4-after-deletion"
	echo "recovery_seconds ${recovery}" >>"${ARTIFACTS}/timings.txt"
	echo "second_ready_seconds ${ready_after}" >>"${ARTIFACTS}/timings.txt"
	pass "${NODE_B} published ${NODE_B_CAPACITY} again ${recovery}s after DeleteVolume of ${pv}"

	at_most "${ready_after}" "${RECOVERY_TIMEOUT_SECONDS}" ||
		fail "pod second became ready ${ready_after}s after DeleteVolume, the limit is ${RECOVERY_TIMEOUT_SECONDS}s"
	pass "PVC second is Bound and pod second is Running on ${NODE_B} ${ready_after}s after DeleteVolume"

	local reduced
	reduced=$(($(to_bytes "${NODE_B_CAPACITY}") - needed))
	wait_until "${PUBLICATION_TIMEOUT_SECONDS}" "capacity of ${NODE_B} reduced" capacity_is "${NODE_B}" "${reduced}" ||
		fail "${NODE_B} does not publish ${reduced} bytes"
	record_capacity "5-second-allocated"
}

test_restart() {
	info "restarting the driver DaemonSet"
	kubectl exec -n "${test_namespace}" second -- sh -c 'echo before-restart > /data/marker && sync' ||
		fail "the volume of pod second is not writable before the restart"
	local before uids_before
	before=$(for node in "${NODE_A}" "${NODE_B}"; do published_capacity "${node}"; done)
	uids_before=$(kubectl get csistoragecapacities -n "${driver_namespace}" -o jsonpath='{.items[*].metadata.uid}' | tr ' ' '\n' | sort)
	driver_logs "${NODE_A}" before-restart
	driver_logs "${NODE_B}" before-restart

	local restarted
	restarted=$(now)
	kubectl rollout restart -n "${driver_namespace}" "daemonset/${daemonset}" >/dev/null
	kubectl rollout status -n "${driver_namespace}" "daemonset/${daemonset}" --timeout=300s >/dev/null
	echo "restart_rollout_seconds $(elapsed_since "${restarted}")" >>"${ARTIFACTS}/timings.txt"

	# The new provisioners publish again within one polling interval.
	sleep "${POLL_INTERVAL_SECONDS}"
	local after uids_after
	after=$(for node in "${NODE_A}" "${NODE_B}"; do published_capacity "${node}"; done)
	[[ "${after}" == "${before}" ]] || fail "capacity changed across the restart: '${before}' -> '${after}'"
	uids_after=$(kubectl get csistoragecapacities -n "${driver_namespace}" -o jsonpath='{.items[*].metadata.uid}' | tr ' ' '\n' | sort)
	[[ "${uids_after}" == "${uids_before}" ]] || fail "the CSIStorageCapacity objects were recreated by the restart"
	record_capacity "6-after-driver-restart"
	pass "the capacity converged to the same value within one polling interval (${POLL_INTERVAL_SECONDS}s) after the restart, objects kept"

	local reserved
	reserved=$(kubectl logs -n "${driver_namespace}" "$(driver_pod "${NODE_B}")" -c kata-directvolume |
		grep -c "$(to_bytes "${SECOND_VOLUME_SIZE}") of $(to_bytes "${NODE_B_CAPACITY}") bytes are reserved from capacity pool \"${CAPACITY_POOL}\"" || true)
	[[ "${reserved}" -eq 1 ]] || fail "the restarted driver on ${NODE_B} did not restore the reservation of PVC second"
	pass "the restarted driver restored the ${SECOND_VOLUME_SIZE} reservation from its state, counted once"

	[[ "$(kubectl exec -n "${test_namespace}" second -- cat /data/marker)" == "before-restart" ]] ||
		fail "the data of pod second is not readable after the restart"
	kubectl exec -n "${test_namespace}" second -- sh -c 'echo after-restart > /data/marker && sync' ||
		fail "the volume of pod second is not writable after the restart"
	pass "the volume of pod second stays readable and writable across the driver restart"

	# The reserved space is not handed out again.
	create_workload third "${FIRST_VOLUME_SIZE}"
	wait_until 60 "pod third is unschedulable" pod_unschedulable third ||
		fail "pod third was scheduled although only $(published_capacity "${NODE_B}") is free on ${NODE_B}"
	[[ -z "$(pv_of third)" ]] || fail "PVC third was provisioned"
	kubectl delete -n "${test_namespace}" pod/third pvc/third --wait=false >/dev/null
	pass "a ${FIRST_VOLUME_SIZE} volume stays pending after the restart: reserved space is not advertised as free"
}

run_tests() {
	mkdir -p "${ARTIFACTS}/logs"
	printf 'phase\tnode\tcapacity\tmaximumVolumeSize\n' >"${timeline}"
	: >"${ARTIFACTS}/timings.txt"
	select_nodes
	reset_test_state
	kubectl create namespace "${test_namespace}" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
	load_image "${TEST_IMAGE}"
	trap stop_capacity_watch EXIT
	start_capacity_watch

	local deployed
	deployed=$(now)
	test_publication "${deployed}"
	test_rbac
	test_scheduling
	test_exhaustion
	test_recovery
	test_restart
	stop_capacity_watch
	info "all checks passed"
}

################################################################################
# Report
################################################################################

report() {
	mkdir -p "${ARTIFACTS}/logs"
	select_nodes
	local node
	for node in "${NODE_A}" "${NODE_B}"; do
		driver_logs "${node}" final
	done
	kubectl get csidriver "${driver_name}" -o yaml >"${ARTIFACTS}/csidriver.yaml"
	kubectl get csistoragecapacity -A -o yaml >"${ARTIFACTS}/csistoragecapacity.yaml"
	kubectl get configmap -n "${driver_namespace}" "${capacity_configmap}" -o yaml >"${ARTIFACTS}/capacity-configmap.yaml"
	kubectl get storageclass "${storage_class}" -o yaml >"${ARTIFACTS}/storageclass.yaml"
	kubectl get pvc,pod -n "${test_namespace}" -o wide >"${ARTIFACTS}/workloads.txt" 2>&1
	kubectl get pvc,pod -n "${test_namespace}" -o yaml >"${ARTIFACTS}/workloads.yaml" 2>&1
	kubectl get pv -o yaml >"${ARTIFACTS}/pv.yaml"
	kubectl get pv -o custom-columns='NAME:.metadata.name,CLAIM:.spec.claimRef.name,CAPACITY:.spec.capacity.storage,NODE-AFFINITY:.spec.nodeAffinity.required.nodeSelectorTerms[*].matchExpressions[*].values' \
		>"${ARTIFACTS}/pv-node-affinity.txt"
	kubectl get events -A --sort-by=.lastTimestamp -o wide >"${ARTIFACTS}/events.txt"
	kubectl get nodes -o wide --show-labels >"${ARTIFACTS}/nodes.txt"
	kubectl get pods -n "${driver_namespace}" -o wide >"${ARTIFACTS}/driver-pods.txt"
	local scheduler
	scheduler=$(kubectl get pods -n kube-system -l component=kube-scheduler -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || true)
	[[ -n "${scheduler}" ]] && kubectl logs -n kube-system "${scheduler}" >"${ARTIFACTS}/logs/kube-scheduler.log" 2>&1

	local provisioner_image driver_image k8s_version
	provisioner_image=$(kubectl get ds -n "${driver_namespace}" "${daemonset}" -o jsonpath='{.spec.template.spec.containers[?(@.name=="csi-provisioner")].image}')
	driver_image=$(kubectl get ds -n "${driver_namespace}" "${daemonset}" -o jsonpath='{.spec.template.spec.containers[?(@.name=="kata-directvolume")].image}')
	k8s_version=$(kubectl version -o json | jq -r '.serverVersion.gitVersion')

	{
		echo "# csi-kata-directvolume storage capacity E2E report"
		echo
		echo "Generated $(date -u +%Y-%m-%dT%H:%M:%SZ) by test/e2e/run-capacity-e2e.sh"
		echo
		echo "## Versions"
		echo
		echo "| Component | Version |"
		echo "| --- | --- |"
		echo "| Kata Containers (kata-deploy) | ${KATA_VERSION}, runtime class \`${runtime_class}\`, guest kernel $(cat "${ARTIFACTS}/kata-guest-kernel.txt" 2>/dev/null || echo n/a) |"
		echo "| csi-kata-directvolume | \`${driver_image}\` built from $(source_revision) |"
		echo "| external-provisioner | \`${provisioner_image}\` |"
		echo "| Kubernetes | ${k8s_version} |"
		echo "| Test pods | \`${TEST_IMAGE}\` |"
		[[ "${USE_EXISTING_CLUSTER}" != "true" ]] && echo "| kind | $(kind version 2>/dev/null) |"
		echo "| Host kernel | $(uname -r) |"
		echo
		echo "## Per-node capacity configuration"
		echo
		echo "ConfigMap \`${driver_namespace}/${capacity_configmap}\`, one key per node, mounted at"
		echo "\`/etc/csi-kata-directvolume/capacity\` and read with \`--capacity-config-dir\`:"
		echo
		echo '```yaml'
		kubectl get configmap -n "${driver_namespace}" "${capacity_configmap}" -o jsonpath='{.data}' | jq .
		echo '```'
		echo
		echo "## Results"
		echo
		echo '```text'
		cat "${results}"
		echo '```'
		echo
		echo "## Capacity values (bytes)"
		echo
		echo '```text'
		column -t -s $'\t' "${timeline}"
		echo '```'
		echo
		echo "## Published capacity over time"
		echo
		echo "Every change of the CSIStorageCapacity objects of \`${storage_class}\`, as seen by \`kubectl get --watch\`:"
		echo
		echo '```text'
		{
			printf 'time (UTC)\tevent\tnode\tcapacity\tmaximumVolumeSize\n'
			awk -v sc="${storage_class}" '$3 == sc {
				cmd = "date -u -d @" $1 " +%H:%M:%S.%3N"; cmd | getline t; close(cmd)
				printf "%s\t%s\t%s\t%s\t%s\n", t, $2, $4, $5, $6
			}' "${watch_log}" 2>/dev/null
		} | column -t -s $'\t'
		echo '```'
		echo
		echo "## Timings (seconds)"
		echo
		echo '```text'
		cat "${ARTIFACTS}/timings.txt"
		echo '```'
		echo
		echo "## PVC, PV and pod placement"
		echo
		local snapshot_file
		for snapshot_file in "${ARTIFACTS}"/snapshots/*.txt; do
			[[ -f "${snapshot_file}" ]] || continue
			echo "### $(basename "${snapshot_file}" .txt)"
			echo
			echo '```text'
			cat "${snapshot_file}"
			echo '```'
			echo
		done
		echo "### End of the test"
		echo
		echo '```text'
		cat "${ARTIFACTS}/workloads.txt"
		echo
		cat "${ARTIFACTS}/pv-node-affinity.txt"
		echo '```'
		echo
		echo "## Scheduling and provisioning events"
		echo
		echo '```text'
		kubectl get events -n "${test_namespace}" --sort-by=.lastTimestamp \
			-o custom-columns='LAST:.lastTimestamp,COUNT:.count,REASON:.reason,OBJECT:.involvedObject.name,SOURCE:.source.component,MESSAGE:.message' 2>&1
		echo '```'
		echo
		echo "Full outputs: csidriver.yaml, csistoragecapacity.yaml, workloads.yaml, pv.yaml, events.txt and logs/."
	} >"${ARTIFACTS}/report.md"
	info "report written to ${ARTIFACTS}/report.md"
}

teardown() {
	if [[ "${USE_EXISTING_CLUSTER}" == "true" || "${KEEP_CLUSTER}" == "true" ]]; then
		kubectl delete namespace "${test_namespace}" --wait=false >/dev/null 2>&1 || true
		kubectl delete storageclass "${storage_class}" >/dev/null 2>&1 || true
		return
	fi
	kind delete cluster --name "${KIND_CLUSTER_NAME}"
	# The raw disks were written by the kind nodes, as root.
	if [[ -d "${NODE_STORAGE_DIR}" ]]; then
		rm -rf "${NODE_STORAGE_DIR}" 2>/dev/null || sudo rm -rf "${NODE_STORAGE_DIR}"
	fi
}

main() {
	local action="${1:-all}"
	mkdir -p "${ARTIFACTS}"
	# The results of setup and test, the report reads them.
	[[ "${action}" == "report" || "${action}" == "teardown" ]] || : >"${results}"
	case "${action}" in
	all)
		setup
		# On the left of || errexit would be off for all the tests. In a
		# subshell it stays on, and die does not skip the report.
		local rc
		set +o errexit
		(
			set -o errexit
			run_tests
		)
		rc=$?
		set -o errexit
		[[ "${rc}" -eq 0 ]] || printf 'FAIL\tthe test stopped with exit status %s\n' "${rc}" >>"${results}"
		report || true
		[[ "${rc}" -eq 0 && "${KEEP_CLUSTER}" != "true" ]] && teardown
		return "${rc}"
		;;
	setup) setup ;;
	test) run_tests ;;
	report) report ;;
	teardown) teardown ;;
	*) die "usage: $0 [all|setup|test|report|teardown]" ;;
	esac
}

main "$@"
