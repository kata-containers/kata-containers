#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for nodeBinaries. No cluster required.
#
# These containers write into /usr/local/bin on every node the install selects,
# from images kata-deploy does not build. What the chart renders for them is the
# difference between a node gaining a binary it lacks and a node losing one
# something else put there, and none of it surfaces until an install runs on a
# real node. The invariants below are the ones a values or template change could
# undo while still rendering something that looks right.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

# Two entries, so that anything per-entry is covered by more than one of them.
NODE_BINARIES=(
	--set deploymentMode=job
	--set 'snapshotter.setup[0]=erofs'
	--set 'nodeBinaries.erofs-utils.image=quay.io/kata-containers/erofs-utils:1.9.3'
	--set 'nodeBinaries.erofs-utils.binaries[0]=mkfs.erofs'
	--set 'nodeBinaries.erofs-utils.binaries[1]=dump.erofs'
	--set 'nodeBinaries.extra-tools.image=quay.io/example/tools:1'
	--set 'nodeBinaries.extra-tools.binaries[0]=thing'
)

# Render the per-node Job templates the dispatcher reads, as one YAML stream.
per_node_jobs() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set deploymentMode=job \
		--show-only templates/kata-deploy-job-templates.yaml \
		"$@"
}

# The pod spec of one stage's per-node Job template, out of the ConfigMap holding
# them. Grepping the whole ConfigMap cannot tell one stage from the other, and
# install and cleanup carry different containers here.
per_node_pod_spec() {
	local stage="${1}"
	shift
	per_node_jobs "$@" | awk -v stage="${stage}" '
		$0 == "  " stage "-job.yaml: |" { inside = 1; next }
		inside && /^  [^ ]/ { inside = 0 }
		inside && $0 == "        spec:" { pod = 1; next }
		pod && /^        [^ ]/ { pod = 0 }
		pod { print }
	'
}

# One named container out of a stage, ending at the next peer container.
stage_container() {
	local stage="${1}" name="${2}"
	shift 2
	per_node_pod_spec "${stage}" "$@" | awk -v want="            - name: ${name}" '
		$0 == want { inside = 1 }
		inside && $0 != want && $0 ~ /^            - name: / { exit }
		inside { print }
	'
}

# The stage's containers, in the order it runs them.
stage_container_names() {
	local stage="${1}"
	shift
	per_node_pod_spec "${stage}" "$@" |
		sed -n 's/^            - name: \(.*\)$/\1/p'
}

# Assert that rendered output does NOT contain a pattern. Not `! grep`: set -e
# ignores its status, so anywhere but the last line of a test it could never fail
# one.
refute_match() {
	local rendered="${1}" pattern="${2}"
	if echo "${rendered}" | grep -q -- "${pattern}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

# Where a line sits in a rendered script, for the tests about what it does
# before what.
line_of() {
	echo "${1}" | grep -nF -- "${2}" | head -1 | cut -d: -f1
}

last_line_of() {
	echo "${1}" | grep -nF -- "${2}" | tail -1 | cut -d: -f1
}

# Why the chart refused to render, for the tests asserting that it does.
render_error() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@" 2>&1 >/dev/null && return 1
}

@test "each nodeBinaries entry stages its binaries in a container of its own" {
	local staging
	staging="$(stage_container install erofs-utils "${NODE_BINARIES[@]}")"

	echo "${staging}" | grep -q 'image: "quay.io/kata-containers/erofs-utils:1.9.3"'
	echo "${staging}" | grep -q 'staged="/node-binaries/erofs-utils"'
	echo "${staging}" | grep -q 'for binary in mkfs.erofs dump.erofs; do'

	# The other entry's, so that one is not standing in for both.
	stage_container install extra-tools "${NODE_BINARIES[@]}" |
		grep -q 'for binary in thing; do'
}

@test "a staging container reaches nothing of the node's" {
	local mounts
	mounts="$(stage_container install erofs-utils "${NODE_BINARIES[@]}" |
		sed -n '/volumeMounts:/,$p')"

	# The pod-local volume it stages into, and nothing else: these run images
	# kata-deploy does not build.
	echo "${mounts}" | grep -q -- '- name: node-binaries'
	refute_match "${mounts}" 'host-'
	refute_match "$(stage_container install erofs-utils "${NODE_BINARIES[@]}")" \
		'privileged: true'
}

@test "the install container takes the staged binaries into the node's /usr/local/bin" {
	local install
	install="$(stage_container install node-binaries-install "${NODE_BINARIES[@]}")"

	echo "${install}" | grep -q 'node_bin=/host-usr-local/bin-writable'
	echo "${install}" | grep -A1 -- '- name: host-usr-local-bin' |
		grep -q 'mountPath: /host-usr-local/bin-writable'
	# Read-only: it installs what was staged, it does not add to it.
	echo "${install}" | grep -A2 -- '- name: node-binaries' | grep -q 'readOnly: true'
	refute_match "${install}" 'privileged: true'
}

@test "the install takes the same node lock as every other host mutation" {
	local stage
	for stage in "install node-binaries-install" "cleanup node-binaries-remove"; do
		# shellcheck disable=SC2086
		local rendered
		rendered="$(stage_container ${stage} "${NODE_BINARIES[@]}")"

		echo "${rendered}" | grep -q 'exec 9>/host-run-lock/kata-deploy.lock'
		echo "${rendered}" | grep -q 'flock -x 9'
		echo "${rendered}" | grep -A1 -- '- name: host-run-lock' |
			grep -q 'mountPath: /host-run-lock'
	done
}

@test "the binaries are installed before the host check looks for them" {
	local names
	names="$(stage_container_names install "${NODE_BINARIES[@]}")"

	# A staging container per entry, then the install, then the check that
	# validates what it installed rather than what it replaced.
	[ "$(echo "${names}" | head -4 | tr '\n' ' ')" = \
		"erofs-utils extra-tools node-binaries-install load-kernel-modules " ]
	echo "${names}" | grep -q '^host-check$'
	[ "$(echo "${names}" | grep -n '^node-binaries-install$' | cut -d: -f1)" -lt \
		"$(echo "${names}" | grep -n '^host-check$' | cut -d: -f1)" ]
}

@test "an install configuring no entry renders none of it" {
	local spec
	spec="$(per_node_pod_spec install --set deploymentMode=job)"

	# No staging container, no install container, and no volume for them to
	# meet in.
	refute_match "${spec}" 'node-binaries'
}

@test "cleanup removes them whether or not an entry is still configured" {
	local without with
	without="$(stage_container cleanup node-binaries-remove --set deploymentMode=job)"
	with="$(stage_container cleanup node-binaries-remove "${NODE_BINARIES[@]}")"

	# Driven by the marker alone, so an uninstall tidies up even when the
	# entries were dropped from the values first.
	for rendered in "${without}" "${with}"; do
		echo "${rendered}" | grep -q 'marker="${node_bin}/.kata-deploy-node-binaries"'
		echo "${rendered}" | grep -q -- '- name: host-usr-local-bin'
		# Nothing staged to install: taking them out is the whole job.
		refute_match "${rendered}" 'mountPath: /node-binaries'
	done
}

@test "the marker naming what to remove is scoped to the installation" {
	# Side-by-side installs each own the set they installed. A shared marker
	# would have the second one remove the first one's binaries.
	stage_container install node-binaries-install "${NODE_BINARIES[@]}" |
		grep -q 'marker="${node_bin}/.kata-deploy-node-binaries"'
	stage_container install node-binaries-install "${NODE_BINARIES[@]}" \
		--set env.multiInstallSuffix=gpu |
		grep -q 'marker="${node_bin}/.kata-deploy-node-binaries-gpu"'
	stage_container cleanup node-binaries-remove --set deploymentMode=job \
		--set env.multiInstallSuffix=gpu |
		grep -q 'marker="${node_bin}/.kata-deploy-node-binaries-gpu"'
}

@test "the previous set is kept until this one is known to be installable" {
	local install
	install="$(stage_container install node-binaries-install "${NODE_BINARIES[@]}")"

	# The one removal that happens before anything is validated belongs to the
	# branch taken when nothing is staged, where removing them is the job.
	local branch first_removal branch_exit
	branch="$(line_of "${install}" 'if [ ! -d "${staged}" ]; then')"
	first_removal="$(line_of "${install}" 'rm -f "${node_bin}/${binary}"')"
	branch_exit="$(line_of "${install}" 'exit 0')"
	[ "${branch}" -lt "${first_removal}" ]
	[ "${first_removal}" -lt "${branch_exit}" ]

	# On the path that installs, a node keeps its working set when the install
	# is refused, so nothing is taken out before every claim is checked.
	local collision last_removal
	collision="$(line_of "${install}" 'was not installed by kata-deploy')"
	last_removal="$(last_line_of "${install}" 'rm -f "${node_bin}/${binary}"')"
	[ "${collision}" -lt "${last_removal}" ]
}

@test "an entry missing what it takes is turned away" {
	render_error --set deploymentMode=job \
		--set 'nodeBinaries.tool.binaries[0]=thing' |
		grep -q 'ERROR: nodeBinaries.tool.image is empty'

	render_error --set deploymentMode=job \
		--set 'nodeBinaries.tool.image=quay.io/example/tools:1' |
		grep -q 'ERROR: nodeBinaries.tool.binaries is empty'
}

@test "a binary name a shell would take apart is turned away" {
	# Each name reaches the node as a word of shell and as a line of the marker.
	local name
	for name in 'two words' '*' 'a; touch /tmp/x' '../../etc/passwd' '-rf'; do
		render_error --set deploymentMode=job \
			--set 'nodeBinaries.tool.image=quay.io/example/tools:1' \
			--set-string "nodeBinaries.tool.binaries[0]=${name}" |
			grep -q 'is not a plain file name'
	done
}

@test "an entry name Kubernetes would refuse as a container name is turned away" {
	local with_image=(--set deploymentMode=job
		--set 'nodeBinaries.NAME.image=quay.io/example/tools:1'
		--set 'nodeBinaries.NAME.binaries[0]=thing')

	# The entry names the container staging its binaries, so it has to be a
	# DNS-1123 label, short enough, and not one of ours.
	render_error "${with_image[@]/NAME/-leading-dash}" |
		grep -q 'not usable as a container name'
	render_error "${with_image[@]/NAME/$(printf 'x%.0s' $(seq 64))}" |
		grep -q 'characters long'
	render_error "${with_image[@]/NAME/host-check}" |
		grep -q 'container kata-deploy runs itself'
}

@test "nodeBinaries is turned away in daemonset mode, which cannot stage them" {
	# The staging containers belong to the job pipeline, and daemonset mode
	# renders none of them: without this the value would be silently ignored.
	render_error --set deploymentMode=daemonset \
		--set 'nodeBinaries.tool.image=quay.io/example/tools:1' \
		--set 'nodeBinaries.tool.binaries[0]=thing' |
		grep -q 'requires deploymentMode: job'
}
