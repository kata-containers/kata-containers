#!/usr/bin/env bats
#
# Copyright (c) 2023 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

RESOURCE_PATH_BASE="/opt/confidential-containers/kbs/repository/"
RESOURCE_ONE_ID="default/resource/one"
RESOURCE_ONE="resource_one"

add_kernel_params() {
    local params="$@"
    load_runtime_config_path

    sudo sed -i -e 's#^\(kernel_params\) = "\(.*\)"#\1 = "\2 '"$params"'"#g' \
        "$RUNTIME_CONFIG_PATH"
}

setup_cc_kbs() {
	# checkout kbs
	kbs_dir=$(mktemp -d /tmp/kbs.XXXX)
	pushd "$kbs_dir"
	git clone https://github.com/confidential-containers/kbs.git 
	pushd kbs

	# configure KBS
	openssl genpkey -algorithm ed25519 > config/private.key
	openssl pkey -in config/private.key -pubout -out config/public.pub

	# build and start kbs
	docker compose build
	docker compose up -d
	
	popd
	popd
}

stop_cc_kbs() {
	pushd "$kbs_dir"
	pushd kbs

	docker compose down
	docker compose remove

	popd
	rm -rf kbs
	popd

}

setup() {
	extract_kata_env
	get_pod_config_dir

	case $KATA_HYPERVISOR in
		qemu-snp | qemu-tdx)
			kbc="cc_kbc"
			setup_cc_kbs

			# add resource to KBS
			resource_one_path="$RESOURCE_PATH_BASE$RESOURCE_ONE_ID"
			mkdir -p "$resource_one_path" 
			echo -n "$RESOURCE_ONE" >> resource_one_path
			;;
		*)
			skip "KBS not supported"
		;;
	esac

	# set aa_kbc_params to point to the KBS that was started
	kbs_ip="$(ip -o route get to 8.8.8.8 | sed -n 's/.*src \([0-9.]\+\).*/\1/p')"
	aa_kbc_params="agent.aa_kbc_params=$kbc::$kbs_ip:8080"
	add_kernel_params "$aa_kbc_params"
	
}

@test "Request resource from Attestation Agent" {

	# Create pod
	pod_name="pod-confidential-resource"
	ctr_name="ctr-confidential-resource"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/pod-confidential-resource.yaml" "$pod_config"
	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	kubectl create -f "${pod_config}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# get a resource. the AA ignores the KbsUri set here
	kubectl exec $pod_name -- grpcurl -plaintext -proto getresource.proto -d "\{\"ResourcePath\":\"$RESOURCE_ONE_ID\",\"KbcName\":\"$kbc\",\"KbsUri\"\:\"0\"\}" 127.0.0.1:50001 getresource.GetResourceService/GetResource | grep "$RESOURCE_ONE"

}

teardown() {

	case $KATA_HYPERVISOR in
		qemu-snp | qemu-tdx)
			stop_cc_kbc

			# cleanup resources
			rm -rf "$resource_one_path" 
			;;
		*)
			skip "KBS not supported"
		;;
	esac

	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	rm -f "$pod_config"


}
