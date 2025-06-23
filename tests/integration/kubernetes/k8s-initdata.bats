#!/usr/bin/env bats
# Copyright (c) 2025 Alibaba Cloud
#
# SPDX-License-Identifier: Apache-2.0
#

# This test we will test initdata in the following logic
# 1. Enable image signature verification via kernel commandline
# 2. Set Trustee address via initdata
# 3. Pull an image from a banned registry
# 4. Check if the pulling fails with log `image security validation failed`,
# the initdata works.
#
# Note that if initdata does not work, the pod still fails to launch (hang at
# CreatingContainer status). The error information is
# `[CDH] [ERROR]: Get Resource failed` which internally means that the KBS URL
# has not been set correctly.
#
# TODO: After https://github.com/kata-containers/kata-containers/issues/9266
# is resolved, both KBS URI and policy URI can be set via initdata.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KBS="${KBS:-false}"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

setup() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common || die "setup_common failed"

    FAIL_TEST_IMAGE="quay.io/prometheus/busybox:latest"

    SECURITY_POLICY_KBS_URI="kbs:///default/security-policy/test"
}

function setup_kbs_image_policy_for_initdata() {
    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    export CURRENT_ARCH=$(uname -m)
    case "${CURRENT_ARCH}" in
        "x86_64"|"s390x")
            ;;
        *)
            skip "Test skipped as only x86-64 & s390x is supported, while current platform is ${CURRENT_ARCH}"
            ;;
    esac

    case "$KATA_HYPERVISOR" in
        "qemu-tdx"|"qemu-coco-dev"|"qemu-coco-dev-runtime-rs"|"qemu-snp"|"qemu-se"|"qemu-se-runtime-rs")
            ;;
        *)
            skip "Test not supported for ${KATA_HYPERVISOR}."
            ;;
    esac

    default_policy="${1:-insecureAcceptAnything}"
    policy_json=$(cat << EOF
{
    "default": [
        {
        "type": "${default_policy}"
        }
    ],
    "transports": {
        "docker": {
            "quay.io/prometheus": [
                {
                    "type": "reject"
                }
            ]
        }
    }
}
EOF
    )

    if ! is_confidential_hardware; then
        kbs_set_allow_all_resources
    fi

    kbs_set_resource "default" "security-policy" "test" "${policy_json}"
}

@test "Test that creating a container from an rejected image configured by initdata, fails according to policy reject" {
    setup_kbs_image_policy_for_initdata

    CC_KBS_ADDRESS=$(kbs_k8s_svc_http_addr)

    kernel_parameter="agent.image_policy_file=${SECURITY_POLICY_KBS_URI} agent.enable_signature_verification=true"
    initdata_annotation=$(gzip -c << EOF | base64 -w0
version = "0.1.0"
algorithm = "sha256"
[data]
"aa.toml" = '''
[token_configs]
[token_configs.kbs]
url = "${CC_KBS_ADDRESS}"
'''

"cdh.toml" = '''
[kbc]
name = "cc_kbc"
url = "${CC_KBS_ADDRESS}"
'''

"policy.rego" = '''
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

package agent_policy

default AddARPNeighborsRequest := true
default AddSwapRequest := true
default CloseStdinRequest := true
default CopyFileRequest := true
default CreateContainerRequest := true
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default ExecProcessRequest := true
default GetMetricsRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default ListInterfacesRequest := true
default ListRoutesRequest := true
default MemHotplugByProbeRequest := true
default OnlineCPUMemRequest := true
default PauseContainerRequest := true
default PullImageRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default ReseedRandomDevRequest := true
default ResumeContainerRequest := true
default SetGuestDateTimeRequest := true
default SetPolicyRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StartTracingRequest := true
default StatsContainerRequest := true
default StopTracingRequest := true
default TtyWinResizeRequest := true
default UpdateContainerRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true
'''
EOF
    )
    create_coco_pod_yaml_with_annotations "${FAIL_TEST_IMAGE}" "${kernel_parameter}" "${initdata_annotation}" "${node}"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" "Image policy rejected: Denied by policy"
}

@test "Test that creating a container from an rejected image not configured by initdata, fails according to CDH error" {
    setup_kbs_image_policy_for_initdata

    kernel_parameter="agent.image_policy_file=${SECURITY_POLICY_KBS_URI} agent.enable_signature_verification=true"

    create_coco_pod_yaml_with_annotations "${FAIL_TEST_IMAGE}" "${kernel_parameter}" "" "${node}"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    if k8s_create_pod "${kata_pod}" ; then
        echo "Expected failure, but pod ${kata_pod} launched successfully."
        return 1
    fi

    assert_logs_contain "${node}" kata "${node_start_time}" "\[CDH\] \[ERROR\]: Image Client error: Initialize resource provider failed: Get resource failed"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    teardown_common "${node}" "${node_start_time:-}"
}
