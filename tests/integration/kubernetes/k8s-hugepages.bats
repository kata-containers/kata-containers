#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"

source "/etc/os-release" || source "/usr/lib/os-release"
issue="https://github.com/kata-containers/tests/issues/4474"
cgroupv2_issue="https://github.com/kata-containers/tests/issues/5218"

setup() {
	extract_kata_env

	pod_name="hugepage-pod"
	get_pod_config_dir

	hugepages_sysfs_dir=hugepages-2048kB
	if [ "$(uname -m)" = s390x ]; then
		# Hugepage size(s) must be set at boot on s390x. Use first that is available.
		# kvm.hpage=1 must also be set.
		hugepages_sysfs_dir=$(ls /sys/kernel/mm/hugepages | head -1)
	fi
	# Hugepages size in bytes
	# Pattern substitute only directly supported in gawk, not mawk -- use sed
	hugepages_size=$(<<< "$hugepages_sysfs_dir" sed -E 's/hugepages-(.+)B/\1/' | awk '{print toupper($0)}' | numfmt --from=iec)
	# Hugepages size as specified by mount(8) (IEC)
	hugepages_size_mount=$(<<< "$hugepages_size" numfmt --to=iec --format %.0f)
	# Hugepages size as asked for by k8s (IEC with `i` suffix)
	hugepages_size_k8s=$(<<< "$hugepages_size" numfmt --to=iec-i --format %.0f)
	# 4G of hugepages in total
	hugepages_count=$(<<< "(4 * 2^30) / $hugepages_size" bc)

	sed "s/\${hugepages_size}/$hugepages_size_k8s/" "$pod_config_dir/pod-hugepage.yaml" > "$pod_config_dir/test_hugepage.yaml"

	# Enable hugepages
	sed -i 's/#enable_hugepages = true/enable_hugepages = true/g' ${RUNTIME_CONFIG_PATH}

	old_pages=$(cat "/sys/kernel/mm/hugepages/$hugepages_sysfs_dir/nr_hugepages")

	sync
	echo 3 > /proc/sys/vm/drop_caches
	echo "$hugepages_count" > "/sys/kernel/mm/hugepages/$hugepages_sysfs_dir/nr_hugepages"

	systemctl restart kubelet
}

@test "Hugepages" {
    [ "${NAME}" == "Ubuntu" ] && [ "$(echo "${VERSION_ID} >= 22.04" | bc -q)" == "1" ] && skip "hugepages test is not working with cgroupsv2 see $cgroupv2_issue"

	# Create pod
	kubectl create -f "${pod_config_dir}/test_hugepage.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Some `mount`s will indicate a total size, hence the .*
	kubectl exec $pod_name mount | grep "nodev on /hugepages type hugetlbfs (rw,relatime,pagesize=$hugepages_size_mount.*)"
}


@test "Hugepages and sandbox cgroup" {
	skip "test not working see: $issue"

	# Enable sandbox_cgroup_only
	# And set default memory to a low value that is not smaller then container's request
	sed -i 's/sandbox_cgroup_only=false/sandbox_cgroup_only=true/g' ${RUNTIME_CONFIG_PATH}
	sed -i 's|^default_memory.*|default_memory = 512|g' $RUNTIME_CONFIG_PATH

	# Create pod
	kubectl create -f "${pod_config_dir}/test_hugepage.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	kubectl exec $pod_name mount | grep "nodev on /hugepages type hugetlbfs (rw,relatime,pagesize=$hugepages_size_mount.*)"

	# Disable sandbox_cgroup_only
	sed -i 's/sandbox_cgroup_only=true/sandbox_cgroup_only=false/g' ${RUNTIME_CONFIG_PATH}
}

teardown() {
	echo "$old_pages" > "/sys/kernel/mm/hugepages/$hugepages_sysfs_dir/nr_hugepages"

	rm "$pod_config_dir/test_hugepage.yaml"
	kubectl delete pod "$pod_name"

	# Disable sandbox_cgroup_only, in case previous test failed.
	sed -i 's/sandbox_cgroup_only=true/sandbox_cgroup_only=false/g' ${RUNTIME_CONFIG_PATH}

	# Disable hugepages and set default memory back to 2048Mi
	sed -i 's/enable_hugepages = true/#enable_hugepages = true/g' ${RUNTIME_CONFIG_PATH}
	sed -i 's|^default_memory.*|default_memory = 2048|g' $RUNTIME_CONFIG_PATH
}
