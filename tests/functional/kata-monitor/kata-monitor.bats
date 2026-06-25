#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# kata-monitor helm chart functional test.
#
# Validates that the optional kata-monitor DaemonSet shipped by the
# kata-deploy helm chart actually rolls out, exercises the per-PR
# kata-monitor image, and exposes per-sandbox Prometheus metrics for a
# live kata pod.
#
# Required environment variables (mirroring kata-deploy.bats):
#   DOCKER_REGISTRY                - Registry for kata-deploy image
#   DOCKER_REPO                    - Repository name for kata-deploy image
#   DOCKER_TAG                     - kata-deploy image tag to test
#   KATA_HYPERVISOR                - Hypervisor to test (qemu, ...)
#   KUBERNETES                     - K8s distribution (k3s, k0s, ...)
#   KATA_MONITOR_IMAGE_REFERENCE   - Registry/repo for the kata-monitor image
#   KATA_MONITOR_IMAGE_TAG         - kata-monitor image tag to test
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

# Reuse the helm install/uninstall helpers maintained alongside the
# kata-deploy bats tests.
source "${BATS_TEST_DIRNAME}/../kata-deploy/lib/helm-deploy.bash"

# Cache update polling interval is short on the monitor side; 30s is
# generous enough to absorb both the cache refresh and any apiserver
# proxy latency without dragging the test runtime up.
KATA_MONITOR_CACHE_TIMEOUT_S="${KATA_MONITOR_CACHE_TIMEOUT_S:-30}"

# Pod name used to give kata-monitor a real sandbox to surface metrics
# about. Kept fixed so teardown can clean it up unconditionally.
KATA_MONITOR_PROBE_POD="kata-monitor-probe"

setup() {
	ensure_helm

	: "${KATA_MONITOR_IMAGE_REFERENCE:?KATA_MONITOR_IMAGE_REFERENCE must be set}"
	: "${KATA_MONITOR_IMAGE_TAG:?KATA_MONITOR_IMAGE_TAG must be set}"
}

# Hit `path` (e.g. /metrics or /sandboxes) on one of the kata-monitor
# pods via the apiserver pod-proxy. Stdout is the response body; stderr
# / non-zero exit propagate kubectl failures.
kata_monitor_get() {
	local path="$1"
	local pod

	pod="$(kubectl -n "${HELM_NAMESPACE}" get pods \
		-l app.kubernetes.io/name=kata-monitor \
		-o jsonpath='{.items[0].metadata.name}')"
	[[ -n "${pod}" ]] || { echo "no kata-monitor pod found" >&2; return 1; }

	kubectl -n "${HELM_NAMESPACE}" get --raw \
		"/api/v1/namespaces/${HELM_NAMESPACE}/pods/${pod}:8090/proxy${path}"
}

# Block until `predicate` returns 0 against the fresh /metrics output,
# or until ${KATA_MONITOR_CACHE_TIMEOUT_S} elapses. The predicate is a
# bash function name receiving the metrics body on stdin.
wait_for_metrics() {
	local predicate="$1"
	local body
	local deadline=$((SECONDS + KATA_MONITOR_CACHE_TIMEOUT_S))

	while (( SECONDS < deadline )); do
		if body="$(kata_monitor_get /metrics 2>/dev/null)" \
			&& printf '%s' "${body}" | "${predicate}"; then
			return 0
		fi
		sleep 1
	done

	echo "Timed out waiting for kata-monitor /metrics predicate '${predicate}'" >&2
	echo "Last response was:" >&2
	printf '%s\n' "${body:-<none>}" | head -50 >&2
	return 1
}

predicate_has_running_shim() {
	# Match `kata_monitor_running_shim_count <N>` where N >= 1.
	grep -E '^kata_monitor_running_shim_count [1-9][0-9]* *$' >/dev/null
}

predicate_no_running_shim() {
	grep -E '^kata_monitor_running_shim_count 0 *$' >/dev/null
}

predicate_has_shim_metric() {
	# Any kata_shim_* metric line with a non-empty sandbox_id label is
	# enough to prove the per-sandbox scrape path works end-to-end.
	grep -E '^kata_shim_[a-z_]+\{[^}]*sandbox_id="[0-9a-f-]+' >/dev/null
}

@test "kata-monitor helm chart rolls out and exposes per-sandbox metrics" {
	pushd "${repo_root_dir}"

	local helm_timeout="${KATA_DEPLOY_TIMEOUT:-600s}"
	local rollout_timeout="${KATA_MONITOR_ROLLOUT_TIMEOUT:-300s}"

	echo "Installing kata-deploy with monitor.enabled=true ..."
	echo "  kata-monitor image: ${KATA_MONITOR_IMAGE_REFERENCE}:${KATA_MONITOR_IMAGE_TAG}"

	HELM_TIMEOUT="${helm_timeout}" deploy_kata "" \
		--set monitor.enabled=true \
		--set "monitor.image.reference=${KATA_MONITOR_IMAGE_REFERENCE}" \
		--set "monitor.image.tag=${KATA_MONITOR_IMAGE_TAG}"

	echo ""
	echo "::group::kata-monitor DaemonSet rollout"
	kubectl -n "${HELM_NAMESPACE}" rollout status ds/kata-monitor \
		--timeout="${rollout_timeout}"
	echo "::endgroup::"

	kubectl -n "${HELM_NAMESPACE}" wait pod \
		-l app.kubernetes.io/name=kata-monitor \
		--for=condition=Ready --timeout="${rollout_timeout}"

	# Enabling monitor.enabled=true in the same chart deploys kata-monitor
	# alongside kata-deploy, and kata-deploy reconfigures and restarts
	# containerd as part of installing kata. That bounce drops the first
	# kata-monitor instance's containerd connection and costs it a one-off
	# restart, which is expected and not a regression. Now that kata-deploy
	# is Ready (so containerd is settled), restart the DaemonSet and assert
	# the fresh pods stay up without restarts — a genuine crash loop (e.g.
	# the recent glibc/musl mismatch) still fails here because it would
	# never reach Ready or would keep restarting.
	kubectl -n "${HELM_NAMESPACE}" rollout restart ds/kata-monitor
	kubectl -n "${HELM_NAMESPACE}" rollout status ds/kata-monitor \
		--timeout="${rollout_timeout}"

	echo ""
	echo "::group::kata-monitor pods"
	kubectl -n "${HELM_NAMESPACE}" get pods -l app.kubernetes.io/name=kata-monitor -o wide
	echo "::endgroup::"

	kubectl -n "${HELM_NAMESPACE}" wait pod \
		-l app.kubernetes.io/name=kata-monitor \
		--for=condition=Ready --timeout="${rollout_timeout}"

	local restarts
	restarts="$(kubectl -n "${HELM_NAMESPACE}" get pods \
		-l app.kubernetes.io/name=kata-monitor \
		-o jsonpath='{range .items[*]}{.status.containerStatuses[0].restartCount}{"\n"}{end}')"
	while IFS= read -r r; do
		[[ -z "${r}" ]] && continue
		[[ "${r}" -eq 0 ]] || {
			echo "kata-monitor pod restarted ${r} time(s) after containerd settled; failing"
			return 1
		}
	done <<< "${restarts}"

	# Give kata-monitor something real to surface metrics about: a kata
	# pod that just sleeps. Reuses the same image as kata-deploy.bats's
	# verification pod for cache-warmth on the runner.
	local probe_yaml
	probe_yaml=$(mktemp)
	cat > "${probe_yaml}" <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${KATA_MONITOR_PROBE_POD}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: probe
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sh", "-c", "sleep 600"]
EOF

	echo ""
	echo "Creating kata probe pod ..."
	kubectl apply -f "${probe_yaml}"
	rm -f "${probe_yaml}"

	kubectl wait "pod/${KATA_MONITOR_PROBE_POD}" \
		--for=condition=Ready --timeout=300s

	echo ""
	echo "::group::Probe pod status"
	kubectl get "pod/${KATA_MONITOR_PROBE_POD}" -o wide
	echo "::endgroup::"

	# Now the per-sandbox assertions. Wait for the monitor's cache to
	# pick the probe up, then prove a shim metric actually lands in the
	# scrape body.
	wait_for_metrics predicate_has_running_shim
	wait_for_metrics predicate_has_shim_metric
	echo "kata-monitor /metrics surfaced the probe sandbox"

	# /sandboxes is the second public endpoint of kata-monitor; confirm
	# it lists at least one sandbox while the probe pod is alive.
	local sandboxes
	sandboxes="$(kata_monitor_get /sandboxes)"
	echo "::group::/sandboxes response"
	printf '%s\n' "${sandboxes}"
	echo "::endgroup::"
	[[ -n "${sandboxes//[[:space:]]/}" ]] || {
		echo "/sandboxes returned empty body" >&2
		return 1
	}

	# Tear the probe pod down and prove kata-monitor's cache flushes
	# the sandbox out — mirrors is_sandbox_missing_iterate in the
	# host-level test.
	echo ""
	echo "Deleting probe pod and asserting cache invalidates ..."
	kubectl delete "pod/${KATA_MONITOR_PROBE_POD}" --wait=true --timeout=60s

	wait_for_metrics predicate_no_running_shim
	echo "kata-monitor /metrics dropped the probe sandbox after deletion"

	popd
}

teardown() {
	# Best-effort cleanup — the @test deletes the probe pod on the
	# happy path, but a failure between create and delete would leave
	# it behind.
	kubectl delete "pod/${KATA_MONITOR_PROBE_POD}" --ignore-not-found --wait=false 2>/dev/null || true

	uninstall_kata
}
