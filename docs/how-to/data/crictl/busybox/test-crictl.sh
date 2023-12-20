#!/bin/bash

sudo crictl rmp -a -f
UUID=$(uuidgen)
printf "metadata:\n  name: testpod,\n  uid: %s,\n  namespace: default\ndns_config:\n  servers:\n    - 8.8.8.8\n" $UUID > mypod.yaml
echo "=== Pod config: ==="
cat mypod.yaml

printf "metadata:\n  name: testcontainer\nimage:\n  image: docker.io/library/busybox:latest\ncommand:\n- sh\n- -c\n- \"while true; do echo 'Hello World!'; sleep 1; done\"\n" > mycontainer.yaml
echo "=== Container config: ==="
cat mycontainer.yaml

CONTAINERID=$(sudo crictl run -r kata mycontainer.yaml mypod.yaml)
printf "=== Created and started container %s ===\n" $CONTAINERID

printf "\n=== Below is the output from the container ===\n"
sudo crictl exec -i -t "$CONTAINERID" uname -a
printf "=== End of the container output ====\n\n"

sudo crictl rm -f "$CONTAINERID"
printf "=== Removed container %s ===\n" $CONTAINERID
sudo crictl rmp -a -f
printf "=== Removed all pods\n"

# clean up
rm -f mypod.yaml mycontainer.yaml
