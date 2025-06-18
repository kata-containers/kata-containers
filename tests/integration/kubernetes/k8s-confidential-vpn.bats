#!/usr/bin/env bats
# Copyright (c) 2025 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {

    # FIXME: The 0.0.0.0 IP is a placeholder
    # Depending on how the workers are set up, this could be the br0 IP address.
    # TODO ? Kicking this in the background may add noise in the logs.
    kubectl exec \
      -it nebula-lighthouse \
      -- bash /usr/local/bin/start-lighthouse.sh 0.0.0.0 192.168.100.100 &

    # capture lighthouse's IP
    lighthouse_pod_ip=$(kubectl get pod nebula-lighthouse -o jsonpath='{.status.podIP}')

    # TODO add the lighthouse pod IP to the CDH config.
    # This has to be done before starting the pods.

    # Start all 3 pods.
    # TODO add this yaml to the PR.
    # Technically we just need the mesh-receiver and mesh-sender now.
    kubectl apply -f 3-pods.yaml

    # Get their IP addresses.
    mesh_sender_ip=$(kubectl get pod mesh-sender -o jsonpath='{.status.podIP}')
    mesh_receiver_ip=$(kubectl get pod mesh-receiver -o jsonpath='{.status.podIP}')
    receiver_ip=$(kubectl get pod receiver -o jsonpath='{.status.podIP}')

    info "mesh sender ip:   ${mesh_sender_ip}"
    info "mesh receiver ip: ${mesh_receiver_ip}"
    info "receiver ip:      ${receiver_ip}"
}

@test "Test that we can send unencrypted traffic between two pods (sanity check)" {
    sudo tcpdump -i any -s0 -nn -w - &> dump.pcap &
    tcpdump_pid=$!

    # start the receiving end
    # TODO do this in the background
    kubectl exec -it receiver -- iperf3 -s -p 5201 -F /data-recv.txt
    # TODO "sleep"

    # send unencrypted traffic
    kubectl exec -it mesh-sender -- iperf3 -p 5201 -F /data.txt -c ${receiver_ip}

    grep -q "coco traffic" dump.pcap
    [ $? == 0 ]

    kill ${tcpdump_pid}
}

@test "Test that we can send encrypted traffic over the vpn and that this traffic is indecipherable outside of the mesh" {
    sudo tcpdump -i any -s0 -nn -w - &> dump-enc.pcap &
    tcpdump_pid=$!

    # TODO convert the receiver's k8s IP to mesh IP
    #mesh_receiver_vpn_ip=TODO

    # start the receiving end
    # TODO do this in the background
    kubectl exec -it mesh-receiver -- iperf3 -s -p 5201 -F /data-recv.txt
    # TODO "sleep"

    # send encrypted traffic
    kubectl exec -it mesh-sender -- iperf3 -p 5201 -F /data.txt -c ${mesh_receiver_vpn_ip}

    grep -q "coco traffic" dump.pcap
    [ $? != 0 ]

    # XXX Not checking that the receiver actually got the full or partial file,
    # partly because it's not a good tool for writing received traffic to file.

    kill ${tcpdump_pid}
}




teardown() {
    kubectl describe pod mesh-sender || true
    kubectl describe pod mesh-receiver || true
    kubectl describe pod receiver || true
    kubectl delete -f 3-pods.yaml || true

    # TODO tear down any tcpdump procs. better variables to track this are
    # needed
}
