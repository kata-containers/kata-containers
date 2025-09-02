#!/usr/bin/env bats
# Copyright (c) 2025 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

nebula_ip_prefix=192.168.100
lighthouse_ip=${nebula_ip_prefix}.100
vpn_pods_yaml=${FIXTURES_DIR}/pods-vpn.yaml

setup() {

    # FIXME: The 0.0.0.0 IP is a placeholder Depending on how the workers are
    # set up, this could be the br0 IP address.
    kubectl exec \
      -it nebula-lighthouse \
      -- bash /usr/local/bin/start-lighthouse.sh 0.0.0.0 ${lighthouse_ip} &

    # capture lighthouse's IP
    lighthouse_pod_ip=$(kubectl get pod nebula-lighthouse -o jsonpath='{.status.podIP}')

    # TODO add the lighthouse pod IP to the CDH config.
    # This has to be done before starting the pods.

    # Start mesh-receiver and sender pods.
    kubectl apply -f ${vpn_pods_yaml}

    # Get their IP addresses.
    mesh_sender_ip=$(kubectl get pod mesh-sender -o jsonpath='{.status.podIP}')
    mesh_receiver_ip=$(kubectl get pod mesh-receiver -o jsonpath='{.status.podIP}')

    info "mesh sender ip:   ${mesh_sender_ip}"
    info "mesh receiver ip: ${mesh_receiver_ip}"
}

@test "Test that we can send unencrypted traffic between two pods (sanity check)" {
    sudo tcpdump -i any -s0 -nn -w - &> dump.pcap &
    tcpdump_pid=$!

    # Start the receiving end
    # FIXME kubectl-exec in background + sleep?
    kubectl exec -it mesh-receiver -- iperf3 -s -p 5201 -F /data-recv.txt &
    recv_pid=$!
    sleep 1

    # Send unencrypted traffic
    kubectl exec -it mesh-sender -- iperf3 -p 5201 -F /data.txt -c ${mesh_receiver_ip}

    grep -q "coco traffic" dump.pcap
    [ $? == 0 ]

    kill ${tcpdump_pid}
    kill ${recv_pid}
}

@test "Test that we can send encrypted traffic over the vpn and that this traffic is indecipherable outside of the mesh" {
    sudo tcpdump -i any -s0 -nn -w - &> dump-enc.pcap &
    tcpdump_pid=$!

    # convert the receiver's k8s IP to mesh IP
    ip_suffix=$(echo ${mesh_receiver_ip} | cut -d '.' -f4)
    mesh_receiver_vpn_ip=${nebula_ip_prefix}.${ip_suffix}

    # Start the receiving end
    # FIXME kubectl-exec in background + sleep?
    kubectl exec -it mesh-receiver -- iperf3 -s -p 5201 -F /data-recv.txt &
    recv_pid=$!
    sleep 1

    # Send encrypted traffic
    kubectl exec -it mesh-sender -- iperf3 -p 5201 -F /data.txt -c ${mesh_receiver_vpn_ip}

    grep -q "coco traffic" dump.pcap
    [ $? != 0 ]

    # XXX Not checking that the receiver actually got the full or partial file,
    # partly because it's not a good tool for writing received traffic to file.

    kill ${tcpdump_pid}
    kill ${recv_pid}
}




teardown() {
    kubectl describe pod mesh-sender || true
    kubectl describe pod mesh-receiver || true
    kubectl delete -f ${vpn_pods_yaml} || true
    pkill tcpdump
}
