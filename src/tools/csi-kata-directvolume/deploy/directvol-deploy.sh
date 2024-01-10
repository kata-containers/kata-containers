#!/usr/bin/env bash
#
# Copyright 2017 The Kubernetes Authors.
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

BASE_DIR=$(dirname "$0")

# KUBELET_DATA_DIR can be set to replace the default /var/lib/kubelet.
# All nodes must use the same directory.
default_kubelet_data_dir=/var/lib/kubelet
: ${KUBELET_DATA_DIR:=${default_kubelet_data_dir}}

# namespace kata-directvolume
DIRECTVOL_NAMESPACE="kata-directvolume"

# Some images are not affected by *_REGISTRY/*_TAG and IMAGE_* variables.
# The default is to update unless explicitly excluded.
update_image () {
    case "$1" in socat) return 1;; esac
}

run () {
    echo "$@" >&2
    "$@"
}

# deploy kata directvolume plugin and registrar sidecar
echo "deploying kata directvolume components"
for i in $(ls ${BASE_DIR}/kata-directvolume/csi-directvol-*.yaml | sort); do
    echo "   $i"
    modified="$(cat "$i" | sed -e "s;${default_kubelet_data_dir}/;${KUBELET_DATA_DIR}/;" | while IFS= read -r line; do
        nocomments="$(echo "$line" | sed -e 's/ *#.*$//')"
        if echo "$nocomments" | grep -q '^[[:space:]]*image:[[:space:]]*'; then
            # Split 'image: quay.io/k8scsi/csi-attacher:vx.y.z'
            # into image (quay.io/k8scsi/csi-attacher:vx.y.z),
            # registry (quay.io/k8scsi),
            # name (csi-attacher),
            # tag (vx.y.z).
            image=$(echo "$nocomments" | sed -e 's;.*image:[[:space:]]*;;')
            registry=$(echo "$image" | sed -e 's;\(.*\)/.*;\1;')
            name=$(echo "$image" | sed -e 's;.*/\([^:]*\).*;\1;')
            tag=$(echo "$image" | sed -e 's;.*:;;')

            # Variables are with underscores and upper case.
            varname=$(echo $name | tr - _ | tr a-z A-Z)

            # Now replace registry and/or tag, if set as env variables.
            # If not set, the replacement is the same as the original value.
            # Only do this for the images which are meant to be configurable.
            if update_image "$name"; then
                prefix=$(eval echo \${${varname}_REGISTRY:-${IMAGE_REGISTRY:-${registry}}}/ | sed -e 's;none/;;')
                if [ "$IMAGE_TAG" = "canary" ] &&
                   [ -f ${BASE_DIR}/canary-blacklist.txt ] &&
                   grep -q "^$name\$" ${BASE_DIR}/canary-blacklist.txt; then
                    # Ignore IMAGE_TAG=canary for this particular image because its
                    # canary image is blacklisted in the deployment blacklist.
                    suffix=$(eval echo :\${${varname}_TAG:-${tag}})
                else
                    suffix=$(eval echo :\${${varname}_TAG:-${IMAGE_TAG:-${tag}}})
                fi
                line="$(echo "$nocomments" | sed -e "s;$image;${prefix}${name}${suffix};")"
            fi
            echo "kata-directvolume plugin        using $line" >&2
        fi
        if ! $have_csistoragecapacity; then
            line="$(echo "$line" | grep -v -e 'storageCapacity: true' -e '--enable-capacity')"
        fi
        echo "$line"
    done)"
    if ! echo "$modified" | kubectl apply -f -; then
        echo "modified version of $i:"
        echo "$modified"
        exit 1
    fi
done

wait_for_daemonset () {
    retries=10
    while [ $retries -ge 0 ]; do
        ready=$(kubectl get -n $1 daemonset $2 -o jsonpath="{.status.numberReady}")
        required=$(kubectl get -n $1 daemonset $2 -o jsonpath="{.status.desiredNumberScheduled}")
        if [ $ready -gt 0 ] && [ $ready -eq $required ]; then
            return 0
        fi
        retries=$((retries - 1))
        sleep 3
    done
    return 1
}


# Wait until the DaemonSet is running on all nodes.
if ! wait_for_daemonset ${DIRECTVOL_NAMESPACE} csi-kata-directvol-plugin; then
    echo
    echo "driver not ready"
    echo "Deployment:"
    (set +e; set -x; kubectl describe all,role,clusterrole,rolebinding,clusterrolebinding,serviceaccount,storageclass,csidriver --all-namespaces -l app.kubernetes.io/instance=directvolume.csi.katacontainers.io)
    echo
    echo "Pod logs:"
    kubectl get pods -l app.kubernetes.io/instance=directvolume.csi.katacontainers.io --all-namespaces -o=jsonpath='{range .items[*]}{.metadata.name}{" "}{range .spec.containers[*]}{.name}{" "}{end}{"\n"}{end}' | while read -r pod containers; do
        for c in $containers; do
            echo
            (set +e; set -x; kubectl logs $pod $c)
        done
    done
    exit 1
fi

kubectl get po,ds -A 
