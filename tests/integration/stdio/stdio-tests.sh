#!/bin/bash
#
# Copyright (c) 2024 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will validate stdio with containerd

source "../../common.bash"
source "../../metrics/lib/common.bash"

export TEST_IMAGE="docker.io/library/busybox:latest"
export CONTAINER_ID="hello"
export LOG_FILE="/tmp/stdio-tests-log-file"
export TEST_RUNTIME="io.containerd.run.kata.v2"
export LARGE_FILE_SIZE=1000000000

echo "pull container image"
check_images ${TEST_IMAGE}

teardown() {
	echo "delete the container"
	if sudo ctr t list -q | grep -q "${CONTAINER_ID}"; then
		stop_container
	fi

	if sudo ctr c list -q | grep -q "${CONTAINER_ID}"; then
	  sudo ctr c rm "${CONTAINER_ID}"
	fi
}

stop_container() {
	local cmd
	sudo ctr t kill --signal SIGKILL --all ${CONTAINER_ID}
	# poll for a while until the task receives signal and exit
	cmd='[ "STOPPED" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | awk "{print \$3}")" ]'
	waitForProcess 10 1 "${cmd}"

	echo "check the container is stopped"
	# there is only title line of ps command
	[ "1" == "$(sudo ctr t ps ${CONTAINER_ID} | wc -l)" ]
}

assert_eq() {
  	local actual="$1"
	local expected="$2"

	if [ "$expected" != "$actual" ]; then
		echo "Assertion failed: Expected '$expected', but got '$actual'"
		exit -1
	fi
}

echo "1. Start a container (using terminal)"
unbuffer sudo ctr run --runtime $TEST_RUNTIME --rm -t ${TEST_IMAGE} ${CONTAINER_ID} whoami> $LOG_FILE 
output=$(cat ${LOG_FILE}| tr -d '[:space:]')
assert_eq $output "root"

/usr/bin/expect <<-EOF
set timeout 5
spawn sudo ctr run --runtime $TEST_RUNTIME --rm -t ${TEST_IMAGE} ${CONTAINER_ID} sh

expect "#" 
send "id\r"

expect {
    "uid=0(root) gid=0(root) groups=0(root),10(wheel)" { send_user "Ok\n" }
    timeout { send_user "Failed\n"; exit 1 }
}

send "exit\r"
EOF
teardown

echo "2. Start a container (not using terminal)"
output=$(sudo ctr run --runtime $TEST_RUNTIME --rm ${TEST_IMAGE} ${CONTAINER_ID} whoami)
assert_eq $output root 

/usr/bin/expect <<-EOF
set timeout 5
spawn sudo ctr run --runtime $TEST_RUNTIME --rm ${TEST_IMAGE} ${CONTAINER_ID} sh

send "whoami\r"

expect {
    "root" { send_user "Ok\n" }
    timeout { send_user "Failed\n"; exit 1 }
}

send "exit\r"

EOF

teardown

echo "3. Start a detached container (using terminal)"
sudo ctr run --runtime $TEST_RUNTIME -d -t ${TEST_IMAGE} ${CONTAINER_ID}
read CID IMAGE RUNTIME <<< $(sudo ctr c ls | grep ${CONTAINER_ID})

assert_eq $CID $CONTAINER_ID
assert_eq $IMAGE $TEST_IMAGE
assert_eq $RUNTIME "io.containerd.run.kata.v2"

teardown

echo "4. Execute command (using terminal) in an existing container"
sudo ctr run --runtime $TEST_RUNTIME -d ${TEST_IMAGE} ${CONTAINER_ID}

unbuffer sudo ctr t exec -t --exec-id foobar ${CONTAINER_ID} whoami>$LOG_FILE 
output=$(cat ${LOG_FILE}|head -n 1|tr -d '[:space:]')
echo $output
assert_eq $output "root"

teardown

echo "5. Execute command (not using terminal) in an existing container"
sudo ctr run --runtime $TEST_RUNTIME -d ${TEST_IMAGE} ${CONTAINER_ID}
output=$(sudo ctr t exec --exec-id foobar ${CONTAINER_ID} whoami)
assert_eq $output "root"

teardown

echo "6. Execute command (not using terminal, pipe stdin) in an existing container"
sudo ctr run --runtime $TEST_RUNTIME -d ${TEST_IMAGE} ${CONTAINER_ID}
# Word count
read F1 F2 F3 <<< $(printf "aaa\nbbb\nccc\n" | sudo ctr t exec --exec-id foobar ${CONTAINER_ID} wc)
assert_eq $F1 3
assert_eq $F2 3
assert_eq $F3 12

# Large file count
head -c $LARGE_FILE_SIZE /dev/random > /tmp/input
output=$(cat /tmp/input | wc -c|tr -d '[:space:]')
assert_eq $output $LARGE_FILE_SIZE

output=$(cat /tmp/input | sudo ctr t exec --exec-id foobar ${CONTAINER_ID} wc -c)
assert_eq $output $LARGE_FILE_SIZE

output=$(cat /tmp/input | sudo ctr t exec --exec-id foobar ${CONTAINER_ID} cat | wc -c)
assert_eq $output $LARGE_FILE_SIZE
# Large file copy
cat /tmp/input | sudo ctr t exec --exec-id foobar ${CONTAINER_ID} cat > /tmp/output
diff -q /tmp/input /tmp/output

teardown
