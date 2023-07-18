#!/usr/bin/env bats
#
# Copyright (c) 2023 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
      pod_name="emptydir-rorw"
      get_pod_config_dir
      rw_ctr="c-rw"
      ro_ctr="c-ro"
}

@test "Empty dir volumes - rorw" {
      # Create the pod
      kubectl apply -f "${pod_config_dir}/pod-rorw-emptydir.yaml"

      # Check pod creation
      kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

      # Check volume mounts
      cmd="mount | grep logs"
      kubectl exec -it $pod_name -c "$rw_ctr" -- sh -c "$cmd" | grep -q virtiofs

      mount_check_cmd="mount | grep logs"
      # Check that the mount for rw container is rw:
      kubectl exec -it $pod_name -c $"rw_ctr" -- sh -c "$mount_check_cmd" | grep "rw"

      write_cmd="echo 'fun-times' > /mnt/app/logs/foobar"

      # Ensure that rw container can write:
      kubectl exec -it $pod_name -c "$rw_ctr" -- sh -c "$write_cmd"

      # Check that the mount for ro container is ro:
      kubectl exec -it $pod_name -c "$ro_ctr" -- sh -c "$cmd" | grep "ro"

      # Ensure that ro container cannot write:
      kubectl exec -it $pod_name -c "$ro_ctr" -- sh -c "$write_cmd" | grep -q Read-only

      # Ensure that ro container can read what was written:
      kubectl exec -it $pod_name -c "$ro_ctr" -- sh -c "cat /mnt/app/logs/foobar"  | grep fun-times
}

teardown() {
      kubectl describe pod "$pod_name"
      kubectl delete pod "$pod_name"
}
