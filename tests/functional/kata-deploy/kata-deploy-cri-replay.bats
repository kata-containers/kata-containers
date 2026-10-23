#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"
source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

setup_file() {
	ensure_helm
	export REPLAY_DIR
	REPLAY_DIR=$(mktemp -d)
	# userDropIn is a scalar: preserve the selected profile's storage settings
	# (and any EROFS overrides) rather than replacing them with the probe value.
	local profile user_drop_in
	profile=$(shim_profile_file "${KATA_HYPERVISOR}")
	generate_base_values "${REPLAY_DIR}/base.yaml"
	user_drop_in=$(yq '.containerd.userDropIn // ""' "${REPLAY_DIR}/base.yaml")
	if [[ -z "${user_drop_in}" && -n "${profile}" ]]; then
		user_drop_in=$(yq '.containerd.userDropIn // ""' "${profile}")
	fi
	USER_DROP_IN="${user_drop_in}"$'\n[debug]\nformat = "text"\n' \
		yq -n '.containerd.userDropIn = strenv(USER_DROP_IN)' > "${REPLAY_DIR}/values.yaml"
	# Also accept a test build preloaded into the node's image store.
	deploy_kata "${REPLAY_DIR}/values.yaml" --set deploymentMode=job \
		--set job.ttlSecondsAfterFinished=3600 --set imagePullPolicy=IfNotPresent
}

cri_identity() {
	local unit
	case "${KUBERNETES}" in
		k3s) unit=k3s.service ;;
		rke2) unit=rke2-server.service ;;
		microk8s) unit=snap.microk8s.daemon-containerd.service ;;
		k0s)
			# containerd is a child of k0scontroller, so the controller's
			# identity does not reveal an embedded containerd restart.
			local pid start
			pid=$(run_on_host "chroot /host pgrep -f '^/var/lib/k0s/bin/containerd[[:space:]]'" true true)
			start=$(run_on_host "chroot /host cut -d ' ' -f22 /proc/${pid}/stat" true true)
			printf 'MainPID=%s\nExecMainStartTimestampMonotonic=%s\n' "${pid}" "${start}"
			return
			;;
		*) unit=containerd.service ;;
	esac
	# systemctl must observe the host's PID namespace as well as its filesystem.
	run_on_host "chroot /host systemctl show ${unit} -p MainPID -p ExecMainStartTimestampMonotonic" true true
}

@test "Repeated identical install jobs do not restart a serving CRI" {
	local node machine_id runtime_version template before after uid cid job pod stage
	node=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')
	machine_id=$(kubectl get node "${node}" -o jsonpath='{.status.nodeInfo.machineID}' | xargs)
	runtime_version=$(kubectl get node "${node}" -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	[[ -n "${machine_id}" && -n "${runtime_version}" ]]

	# A chart upgrade may skip satisfied nodes. Replay the actual per-node Job
	# instead, retaining every stage and its original mounts and privileges.
	template=$(kubectl -n "${HELM_NAMESPACE}" get cm \
		-l "app.kubernetes.io/instance=${HELM_RELEASE_NAME}" -o json |
		jq -er '[.items[].data["install-job.yaml"] | select(. != null)] |
			if length == 1 then .[0] else error("expected one install template") end')
	printf '%s\n' "${template}" | yq -o=json > "${REPLAY_DIR}/template.json"

	# The counter is expanded by the workload shell, not the test runner.
	# shellcheck disable=SC2016
	kubectl -n "${HELM_NAMESPACE}" run cri-replay-survivor \
		--image=quay.io/kata-containers/alpine-bash-curl:latest --restart=Never \
		--overrides="{\"spec\":{\"runtimeClassName\":\"kata-${KATA_HYPERVISOR}\",\"nodeSelector\":{\"kubernetes.io/hostname\":\"${node}\"}}}" \
		--command -- sh -c 'n=0; while true; do n=$((n+1)); echo "$n" >/tmp/next; mv /tmp/next /tmp/counter; sleep 1; done'
	kubectl -n "${HELM_NAMESPACE}" wait pod/cri-replay-survivor --for=condition=Ready --timeout=300s
	uid=$(kubectl -n "${HELM_NAMESPACE}" get pod cri-replay-survivor -o jsonpath='{.metadata.uid}')
	cid=$(kubectl -n "${HELM_NAMESPACE}" get pod cri-replay-survivor -o jsonpath='{.status.containerStatuses[0].containerID}')
	[[ -n "${uid}" && -n "${cid}" ]]
	before=$(cri_identity)
	[[ "${before}" =~ MainPID=[1-9][0-9]* ]]
	[[ "${before}" =~ ExecMainStartTimestampMonotonic=[1-9][0-9]* ]]

	# One replay can skip correctly yet rewrite identical files and advance
	# their mtimes. Only a subsequent replay exposes the spurious restart.
	for attempt in 1 2 3; do
		job="cri-install-replay-${attempt}"
		jq --arg name "${job}" --arg ns "${HELM_NAMESPACE}" --arg node "${node}" \
			--arg mid "${machine_id}" --arg rv "${runtime_version}" '
			.metadata.name=$name | .metadata.namespace=$ns |
			.spec.template.spec.nodeName=$node |
			(.spec.template.spec.initContainers[]?, .spec.template.spec.containers[]).env |=
			((. // [] | map(select(.name!="NODE_MACHINE_ID" and .name!="CONTAINER_RUNTIME_VERSION"))) +
			[{"name":"NODE_MACHINE_ID","value":$mid},{"name":"CONTAINER_RUNTIME_VERSION","value":$rv}])
		' "${REPLAY_DIR}/template.json" > "${REPLAY_DIR}/${job}.json"
		kubectl create -f "${REPLAY_DIR}/${job}.json"
		kubectl -n "${HELM_NAMESPACE}" wait "job/${job}" --for=condition=Complete --timeout=600s
		pod=$(kubectl -n "${HELM_NAMESPACE}" get pods -l "job-name=${job}" -o json |
			jq -er '.items[] | select(.status.phase=="Succeeded") | .metadata.name')
		stage=$(jq -er '(.spec.template.spec.initContainers[]?, .spec.template.spec.containers[]) |
			select(((.command // []) + (.args // [])) | index("install-stage-cri")) | .name' \
			"${REPLAY_DIR}/template.json")
		run kubectl -n "${HELM_NAMESPACE}" logs "${pod}" -c "${stage}"
		echo "# Replay ${attempt}: ${output}" >&3
		[[ "${status}" -eq 0 && "${output}" == *"CRI stage complete without restart"* ]]
		after=$(cri_identity)
		[[ "${before}" == "${after}" ]]
		[[ "$(kubectl -n "${HELM_NAMESPACE}" get pod cri-replay-survivor -o jsonpath='{.metadata.uid}')" == "${uid}" ]]
		[[ "$(kubectl -n "${HELM_NAMESPACE}" get pod cri-replay-survivor -o jsonpath='{.status.containerStatuses[0].containerID}')" == "${cid}" ]]
		[[ "$(kubectl -n "${HELM_NAMESPACE}" get pod cri-replay-survivor -o jsonpath='{.status.containerStatuses[0].restartCount}')" == "0" ]]
		local counter next
		counter=$(kubectl -n "${HELM_NAMESPACE}" exec cri-replay-survivor -- cat /tmp/counter)
		sleep 2
		next=$(kubectl -n "${HELM_NAMESPACE}" exec cri-replay-survivor -- cat /tmp/counter)
		[[ "${next}" -gt "${counter}" ]]
	done
}

teardown() {
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" ]]; then
		kubectl -n "${HELM_NAMESPACE}" get jobs,pods -o wide || true
	fi
}

teardown_file() {
	kubectl -n "${HELM_NAMESPACE}" delete pod cri-replay-survivor --ignore-not-found --wait || true
	for attempt in 1 2 3; do
		kubectl -n "${HELM_NAMESPACE}" delete job "cri-install-replay-${attempt}" --ignore-not-found --wait || true
	done
	uninstall_kata
	rm -rf "${REPLAY_DIR:?}"
}
