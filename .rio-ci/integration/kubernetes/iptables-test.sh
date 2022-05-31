#!/bin/bash
set -e

RUNTIME=io.containerd.kata-qemu.v2
RUNTIME_CONFIG="/etc/kata-containers/configuration-qemu.toml"
IMAGE=docker.apple.com/library/busybox:latest
CID=test-iptables

# pull image:
ctr image pull "$IMAGE"

# remove if already exists:
ctr task kill $CID || true
ctr c rm $CID || true

# start the container
ctr run  --runtime "$RUNTIME" --runtime-config-path "$RUNTIME_CONFIG" --rm -d "$IMAGE" "$CID"

# get initial ip tables. We expect a series of results, which ends with COMMMIT. Let's
# just key in on COMMIT, and a successful get:
kata-runtime --config "$RUNTIME_CONFIG" iptables get --sandbox-id "$CID" | grep -q COMMIT

# Successful set of the iptables for the pod:

file=$(mktemp)

cat << EOF > "$file"
*nat
-A PREROUTING -d 192.168.103.153/32 -j DNAT --to-destination 192.168.188.153

COMMIT
EOF

kata-runtime --config "$RUNTIME_CONFIG" iptables set --sandbox-id "$CID" "$file"

# verify the DNAT exists:
kata-runtime --config "$RUNTIME_CONFIG" iptables get --sandbox-id "$CID" | grep -q "PREROUTING.*j\ DNAT"

# Now, verify that garbage input results in an error being returned:
cat << EOF > "$file"
foo
bar*nat
 -A PREROUTING -d 192.168.103.153/32 -j DNAT --to-destination 192.168.188.153

 COMMIT
EOF

if kata-runtime --config "$RUNTIME_CONFIG" iptables set --sandbox-id "$CID" "$file"; then
	echo >&2 "error - expected iptables set to fail"
	exit 1
fi

ctr task kill "$CID"
ctr c rm  "$CID"

