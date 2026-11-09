#!/usr/bin/env bash
# Copyright (c) 2026 Tencent. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0

# Run against a dedicated containerd instance configured for Firecracker VMDK.
set -euo pipefail
: "${CONTAINERD_ADDRESS:?Set the dedicated containerd socket}"
: "${BASE_IMAGE_ARCHIVE:?Set a Docker save archive containing a /bin/sh image}"
CTR=${CTR:-ctr}
CRICTL=${CRICTL:-crictl}
RUNTIME_HANDLER=${RUNTIME_HANDLER:-kata-fc-vmdk}
work=$(mktemp -d)
image="localhost/kata-fc-vmdk:e2e-$$"
pod_id=
container_id=
ctr_cmd=("${CTR}" --address "${CONTAINERD_ADDRESS}" --namespace k8s.io)
cri=("${CRICTL}" --config /dev/null --runtime-endpoint "unix://${CONTAINERD_ADDRESS}" --image-endpoint "unix://${CONTAINERD_ADDRESS}" --timeout 60s)

cleanup() {
    local rc=$?
    trap - EXIT
    if [[ -n ${container_id} ]]; then
        "${cri[@]}" stop "${container_id}" >/dev/null || rc=1
        "${cri[@]}" rm "${container_id}" >/dev/null || rc=1
    fi
    if [[ -n ${pod_id} ]]; then
        "${cri[@]}" stopp "${pod_id}" >/dev/null || rc=1
        "${cri[@]}" rmp "${pod_id}" >/dev/null || rc=1
    fi
    "${cri[@]}" rmi "${image}" >/dev/null || rc=1
    rm -rf -- "${work}"
    exit "${rc}"
}
trap cleanup EXIT

# Add independent layers to a cached image, including an overwrite and whiteout.
python3 - "${BASE_IMAGE_ARCHIVE}" "${work}/image.tar" "${image}" <<'PY'
import hashlib
import io
import json
import sys
import tarfile

source, destination, image = sys.argv[1:]
with tarfile.open(source) as base, tarfile.open(destination, 'w') as output:
    manifest = json.load(base.extractfile('manifest.json'))[0]
    config = json.load(base.extractfile(manifest['Config']))
    for member in base:
        if member.name not in ('manifest.json', 'repositories', 'index.json', 'oci-layout'):
            output.addfile(member, base.extractfile(member) if member.isfile() else None)

    def add(name, data):
        entry = tarfile.TarInfo(name)
        entry.size = len(data)
        entry.mode = 0o644
        output.addfile(entry, io.BytesIO(data))

    payload = bytes(range(256)) * 8192
    layers = [
        {'vmdk-test/lower.txt': b'lower\n', 'vmdk-test/order.txt': b'lower\n',
         'vmdk-test/removed': b'remove me\n', 'vmdk-test/payload': payload},
        {'vmdk-test/order.txt': b'upper\n', 'vmdk-test/.wh.removed': b''},
    ]
    for files in layers:
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode='w') as layer:
            directory = tarfile.TarInfo('vmdk-test')
            directory.type = tarfile.DIRTYPE
            directory.mode = 0o755
            layer.addfile(directory)
            for name, content in files.items():
                entry = tarfile.TarInfo(name)
                entry.size = len(content)
                entry.mode = 0o644
                layer.addfile(entry, io.BytesIO(content))
        data = data.getvalue()
        digest = hashlib.sha256(data).hexdigest()
        name = f'{digest}/layer.tar'
        add(name, data)
        manifest['Layers'].append(name)
        config['rootfs']['diff_ids'].append('sha256:' + digest)
        config.setdefault('history', []).append({'created_by': 'Kata Firecracker VMDK E2E'})
    config_data = json.dumps(config).encode()
    manifest['Config'] = hashlib.sha256(config_data).hexdigest() + '.json'
    manifest['RepoTags'] = [image]
    add(manifest['Config'], config_data)
    add('manifest.json', json.dumps([manifest]).encode())
    print('PAYLOAD_SHA256=' + hashlib.sha256(payload).hexdigest())
PY
"${ctr_cmd[@]}" images import --local --snapshotter erofs "${work}/image.tar"

cat >"${work}/pod.json" <<EOF
{"metadata":{"name":"fc-vmdk-e2e-$$","namespace":"fc-vmdk","uid":"fc-vmdk-e2e-$$","attempt":0},"log_directory":"${work}","linux":{"security_context":{"namespace_options":{"network":2}}}}
EOF
pod_id=$("${cri[@]}" runp --runtime "${RUNTIME_HANDLER}" "${work}/pod.json")

# Reuse the same sandbox to exercise detach and slot reuse as well as boot.
for round in 1 2 3; do
    cat >"${work}/container.json" <<EOF
{"metadata":{"name":"fc-vmdk-${round}","attempt":0},"image":{"image":"${image}"},"command":["/bin/sh","-c","sleep 300"],"log_path":"container-${round}.log","linux":{"resources":{"memory_limit_in_bytes":134217728}}}
EOF
    container_id=$("${cri[@]}" create "${pod_id}" "${work}/container.json" "${work}/pod.json")
    "${cri[@]}" start "${container_id}"
    vm_dir="/run/kata/firecracker/${pod_id}/root"
    api_socket="${vm_dir}/run/fc.sock"
    if [[ ! -S ${api_socket} ]]; then
        vm_dir="/run/kata/${pod_id}/root"
        api_socket="/run/kata/${pod_id}/run/fc.sock"
    fi
    curl --fail --silent --unix-socket "${api_socket}" http://localhost/vm/config >"${work}/fc.json"
    python3 - "${work}/fc.json" "${vm_dir}" <<'PY_CONFIG'
import json
from pathlib import Path
import sys
config = json.loads(Path(sys.argv[1]).read_text())
active = [d for d in config['drives'] if d.get('image_format') == 'Vmdk' and 'vmdk-placeholder' not in d['path_on_host']]
assert len(active) == 1, active
assert active[0]['is_read_only'] and active[0]['io_engine'] == 'Sync', active
sources = list(Path(sys.argv[2]).glob('vmdk-[0-9]*/disk.vmdk'))
assert len(sources) == 1, sources
extents = [line for line in sources[0].read_text().splitlines() if ' FLAT ' in line]
assert len(extents) >= 3, extents
print('PASS: native read-only VMDK drive, Sync engine, GPT with multiple extents')
PY_CONFIG
    # Shell substitutions below run inside the guest container.
    # shellcheck disable=SC2016
    "${cri[@]}" exec "${container_id}" /bin/sh -exc '
        test "$(cat /vmdk-test/lower.txt)" = lower
        test "$(cat /vmdk-test/order.txt)" = upper
        test ! -e /vmdk-test/removed
        test ! -e /vmdk-test/writable
        echo writable >/vmdk-test/writable
        test "$(cat /vmdk-test/writable)" = writable
        echo modified >/vmdk-test/lower.txt
        rm /vmdk-test/order.txt
        test ! -e /vmdk-test/order.txt
        echo "91d3beb88a9b2f778a6c44a1c53b63d3c79931845a9aef84b3fb414610bd1938  /vmdk-test/payload" | sha256sum -c -
        uname -r
    '
    "${cri[@]}" stop "${container_id}"
    "${cri[@]}" rm "${container_id}"
    container_id=
    if compgen -G "${vm_dir}/vmdk-[0-9]*" >/dev/null; then
        echo 'VMDK extent directory leaked after container removal' >&2
        exit 1
    fi
    echo "PASS: VMDK layers, whiteout, writable overlay and slot reuse (round ${round})"
done
