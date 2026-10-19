#!/usr/bin/env python3
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

"""Validate installed CI artifacts and render a last, per-shim TOML drop-in.

Python 3.11+. `self-test` uses only temporary files, with no host/cluster access.
`render INSTALL_ROOT SHIM OUTPUT` never edits an existing file.
`check-cri INSTALL_ROOT SHIM` validates containerd's effective config on stdin.
`rootfs-type INSTALL_ROOT SHIM` prints the effective QEMU rootfs filesystem.
"""

import copy
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import tomllib
import unittest


DROPIN = "99-guest-apparmor-ci.toml"
LSM = "lockdown,yama,loadpin,safesetid,integrity,apparmor,bpf"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def layout(root, shim):
    require(shim in ("qemu", "qemu-runtime-rs"), f"unsupported shim: {shim}")
    root = Path(root)
    require(root.is_absolute(), "install root must be absolute")
    defaults = root / "share/defaults/kata-containers"
    if shim == "qemu-runtime-rs":
        defaults /= "runtime-rs"
    config = defaults / "runtimes" / shim / f"configuration-{shim}.toml"
    binary = root / ("runtime-rs/bin" if shim.endswith("-rs") else "bin")
    return config, defaults / f"configuration-{shim}.toml", binary / "containerd-shim-kata-v2"


def artifact(path, root, executable=False):
    path = Path(path)
    require(path.is_absolute(), f"artifact path must be absolute: {path}")
    resolved = path.resolve(strict=True)
    require(resolved.is_relative_to(root.resolve(strict=True)), f"artifact escapes install root: {path}")
    require(resolved.is_file() and resolved.stat().st_size > 0, f"missing/empty regular artifact: {path}")
    require(os.access(resolved, os.R_OK), f"unreadable artifact: {path}")
    require(not executable or os.access(resolved, os.X_OK), f"artifact is not executable: {path}")
    print(f"artifact: {path} -> {resolved}")
    return path


def verity(path):
    text = path.read_text().strip()
    pairs = text.split(",")
    require(all(pair.count("=") == 1 for pair in pairs), f"malformed verity sidecar: {path}")
    values = dict(pair.split("=", 1) for pair in pairs)
    expected = {"root_hash", "salt", "data_blocks", "data_block_size", "hash_block_size"}
    require(len(values) == len(pairs) and set(values) == expected, f"incomplete/duplicate verity fields: {path}")
    require(re.fullmatch(r"[0-9a-fA-F]{64}", values["root_hash"]), f"invalid verity root hash: {path}")
    require(re.fullmatch(r"(?:[0-9a-fA-F]{2})+|-", values["salt"]), f"invalid verity salt: {path}")
    for key in ("data_blocks", "data_block_size", "hash_block_size"):
        require(re.fullmatch(r"[1-9][0-9]*", values[key]), f"invalid verity {key}: {path}")
    for key in ("data_block_size", "hash_block_size"):
        size = int(values[key])
        require(size >= 512 and size & (size - 1) == 0, f"invalid verity block size: {path}")
    return text


def merge(target, incoming):
    for key, value in incoming.items():
        if isinstance(value, dict) and isinstance(target.get(key), dict):
            merge(target[key], value)
        else:
            target[key] = copy.deepcopy(value)


def rootfs_type(root, shim):
    config, _, _ = layout(root, shim)
    effective = tomllib.loads(config.read_text())
    for entry in sorted(config.parent.joinpath("config.d").iterdir()):
        merge(effective, tomllib.loads(entry.read_text()))
    filesystem = effective["hypervisor"]["qemu"].get("rootfs_type")
    require(filesystem in ("ext4", "erofs"), "QEMU rootfs_type must explicitly select ext4 or erofs")
    return filesystem


def render(root, shim, output):
    root, output = Path(root), Path(output)
    config, alias, shim_binary = layout(root, shim)
    artifact(config, root)
    require(alias.resolve(strict=True) == config.resolve(strict=True), "installed config alias does not select the per-shim config")
    require(output == config.parent / "config.d" / DROPIN, "unexpected CI drop-in destination")
    require(output.parent.is_dir() and not output.parent.is_symlink(), "missing/unsafe per-shim config.d")
    require(not output.exists() and not output.is_symlink(), "refusing to overwrite an existing CI drop-in")
    effective = tomllib.loads(config.read_text())
    # Both runtimes consume the sorted per-shim drop-ins. Reject unrecognised
    # entries, rather than silently ignoring something that affects the guest.
    for entry in sorted(output.parent.iterdir()):
        require(entry.is_file() and entry.name < DROPIN, f"unsafe or later drop-in: {entry}")
        artifact(entry, root)
        merge(effective, tomllib.loads(entry.read_text()))
    qemu = effective["hypervisor"]["qemu"]
    require(not qemu.get("guest_extension_images"), "CI expects no previously configured guest extensions")
    require(qemu.get("rootfs_type") in ("ext4", "erofs"), "QEMU rootfs_type must explicitly select ext4 or erofs")
    artifact(shim_binary, root, executable=True)
    for key in ("path", "kernel"):
        artifact(qemu[key], root, executable=key == "path")
    if qemu.get("shared_fs") in ("virtio-fs", "virtio-fs-nydus"):
        artifact(qemu["virtio_fs_daemon"], root, executable=True)
    for key in ("firmware", "firmware_volume"):
        if qemu.get(key):
            artifact(qemu[key], root)
    images = root / "share/kata-containers"
    base = artifact(images / "kata-containers.img", root)
    extension = artifact(images / "kata-containers-apparmor-extension.img", root)
    base_hash = verity(artifact(images / "root_hash_base.txt", root))
    extension_hash = verity(artifact(images / "root_hash_apparmor-extension.txt", root))
    params = [param for param in qemu.get("kernel_params", "").split()
              if param.split("=", 1)[0] not in ("lsm", "security", "apparmor", "selinux", "init")]
    params += [f"lsm={LSM}", "security=apparmor", "apparmor=1", "init=/sbin/init"]
    # Preserve the deployed QEMU setting. The shell preflight probes p1 with
    # blkid and requires a match, rather than guessing from names/build defaults.
    scalars = {"image": str(base), "initrd": "", "rootfs_type": qemu["rootfs_type"],
               "kernel_verity_params": base_hash, "kernel_params": " ".join(params),
               "disable_guest_selinux": True, "enable_debug": True}
    lines = ["# Disposable GitHub runner CI only; not product defaults.", "[hypervisor.qemu]"]
    lines += [f"{key} = {json.dumps(value)}" for key, value in scalars.items()]
    # The array table MUST follow every QEMU scalar, or TOML changes its scope.
    lines += ["", "[[hypervisor.qemu.guest_extension_images]]", 'name = "apparmor"',
              f"path = {json.dumps(str(extension))}", f"verity_params = {json.dumps(extension_hash)}",
              "", "[agent.kata]", "enable_debug = true", ""]
    text = "\n".join(lines)
    parsed = tomllib.loads(text)
    require(all(parsed["hypervisor"]["qemu"][key] == value for key, value in scalars.items()), "QEMU scalar scope changed")
    with output.open("x") as stream:
        stream.write(text)
    print(text)


def check_cri(root, shim, data):
    config, _, binary = layout(root, shim)
    matches = []
    for plugin in data.get("plugins", {}).values():
        if isinstance(plugin, dict):
            runtime = plugin.get("containerd", {}).get("runtimes", {}).get(f"kata-{shim}")
            if runtime is not None:
                matches.append(runtime)
    require(len(matches) == 1, "expected exactly one effective containerd Kata handler")
    runtime = matches[0]
    require(runtime.get("options", {}).get("ConfigPath") == str(config), "containerd ConfigPath does not select the per-shim config")
    require(runtime.get("runtime_path") == str(binary), "containerd runtime_path does not select the installed shim")
    artifact(config, Path(root))
    artifact(binary, Path(root), executable=True)
    print(f"containerd handler kata-{shim}: {config}; {binary}")


class ConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="kata-apparmor-ci-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.shim = "qemu"
        self.create_fixture(self.shim)

    def create_fixture(self, shim):
        config, alias, binary = layout(self.root, shim)
        config.parent.joinpath("config.d").mkdir(parents=True)
        alias.symlink_to(config.relative_to(alias.parent))
        for name in ("bin/qemu", "bin/virtiofsd", "kernel", "share/kata-containers/kata-containers.img",
                     "share/kata-containers/kata-containers-apparmor-extension.img", str(binary.relative_to(self.root))):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture only; never executed\n")
            path.chmod(0o755)
        self.hash_text = f"root_hash={'a' * 64},salt={'b' * 64},data_blocks=8,data_block_size=4096,hash_block_size=4096"
        for name in ("base", "apparmor-extension"):
            (self.root / f"share/kata-containers/root_hash_{name}.txt").write_text(self.hash_text + "\n")
        config.write_text(f'''[hypervisor.qemu]
path = {json.dumps(str(self.root / "bin/qemu"))}
kernel = {json.dumps(str(self.root / "kernel"))}
rootfs_type = "ext4"
shared_fs = "virtio-fs"
virtio_fs_daemon = {json.dumps(str(self.root / "bin/virtiofsd"))}
kernel_params = "console=ttyS0 lsm=selinux security=selinux apparmor=0 init=/bad"
[runtime]
enable_debug = false
''')
        self.config, self.output = config, config.parent / "config.d" / DROPIN

    def run_render(self):
        render(self.root, self.shim, self.output)
        return tomllib.loads(self.output.read_text())

    def test_both_runtimes_and_toml_scope(self):
        for shim in ("qemu", "qemu-runtime-rs"):
            with self.subTest(shim=shim):
                if shim != self.shim:
                    self.create_fixture(shim)
                self.shim = shim
                qemu = self.run_render()["hypervisor"]["qemu"]
                self.assertEqual(qemu["initrd"], "")
                self.assertEqual(qemu["kernel_verity_params"], self.hash_text)
                self.assertEqual(qemu["guest_extension_images"][0]["verity_params"], self.hash_text)
                self.assertEqual(qemu["kernel_params"].split(), ["console=ttyS0", f"lsm={LSM}", "security=apparmor", "apparmor=1", "init=/sbin/init"])
                self.assertNotIn("kernel_params", qemu["guest_extension_images"][0])

    def test_missing_assets_fail_without_output(self):
        paths = ["kernel", "bin/qemu", "bin/virtiofsd", "bin/containerd-shim-kata-v2",
                 "share/kata-containers/kata-containers.img", "share/kata-containers/root_hash_base.txt",
                 "share/kata-containers/kata-containers-apparmor-extension.img",
                 "share/kata-containers/root_hash_apparmor-extension.txt"]
        for name in paths:
            with self.subTest(asset=name):
                path = self.root / name
                hidden = path.with_suffix(".saved")
                path.rename(hidden)
                try:
                    with self.assertRaises((OSError, ValueError)):
                        self.run_render()
                    self.assertFalse(self.output.exists())
                finally:
                    hidden.rename(path)

    def test_bad_verity_fails(self):
        path = self.root / "share/kata-containers/root_hash_apparmor-extension.txt"
        for bad in ("", "root_hash=abc", self.hash_text + ",salt=aa", self.hash_text.replace("data_blocks=8", "data_blocks=0"), self.hash_text.replace("4096", "513")):
            with self.subTest(verity=bad):
                path.write_text(bad)
                with self.assertRaises(ValueError):
                    self.run_render()
                self.assertFalse(self.output.exists())

    def test_late_dropin_is_rejected(self):
        self.output.with_name("zz-override.toml").write_text('[hypervisor.qemu]\ninitrd="old"\n')
        with self.assertRaisesRegex(ValueError, "later drop-in"):
            self.run_render()

    def test_effective_erofs_setting_is_preserved(self):
        self.output.with_name("10-rootfs.toml").write_text('[hypervisor.qemu]\nrootfs_type="erofs"\n')
        self.assertEqual(self.run_render()["hypervisor"]["qemu"]["rootfs_type"], "erofs")
        self.assertEqual(rootfs_type(self.root, self.shim), "erofs")

    def test_unknown_rootfs_is_rejected(self):
        self.output.with_name("10-rootfs.toml").write_text('[hypervisor.qemu]\nrootfs_type="xfs"\n')
        with self.assertRaisesRegex(ValueError, "rootfs_type"):
            self.run_render()

    def test_existing_output_is_preserved(self):
        self.output.write_text("user change")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.run_render()
        self.assertEqual(self.output.read_text(), "user change")

    def test_existing_extensions_are_rejected(self):
        self.output.with_name("10-existing.toml").write_text('[[hypervisor.qemu.guest_extension_images]]\nname="other"\n')
        with self.assertRaisesRegex(ValueError, "previously configured"):
            self.run_render()

    def test_effective_artifact_override_is_checked(self):
        self.output.with_name("10-paths.toml").write_text(
            f'[hypervisor.qemu]\nkernel={json.dumps(str(self.root / "missing-kernel"))}\n')
        with self.assertRaises(OSError):
            self.run_render()

    def test_escaping_artifact_is_rejected(self):
        path = self.root / "kernel"
        path.unlink()
        path.symlink_to(Path(__file__).resolve())
        with self.assertRaisesRegex(ValueError, "escapes install root"):
            self.run_render()

    def test_cri_v2_v3_and_wrong_path(self):
        for shim in ("qemu", "qemu-runtime-rs"):
            if shim != "qemu":
                self.create_fixture(shim)
            config, _, binary = layout(self.root, shim)
            for plugin in ("io.containerd.grpc.v1.cri", "io.containerd.cri.v1.runtime"):
                handler = {"runtime_path": str(binary), "options": {"ConfigPath": str(config)}}
                data = {"plugins": {plugin: {"containerd": {"runtimes": {f"kata-{shim}": handler}}}}}
                check_cri(self.root, shim, data)
                handler["options"]["ConfigPath"] = "/wrong/configuration.toml"
                with self.assertRaisesRegex(ValueError, "ConfigPath"):
                    check_cri(self.root, shim, data)


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["self-test"]:
            unittest.main(argv=[sys.argv[0]], verbosity=2, buffer=True)
        elif len(sys.argv) == 5 and sys.argv[1] == "render":
            render(*sys.argv[2:])
        elif len(sys.argv) == 4 and sys.argv[1] == "check-cri":
            check_cri(*sys.argv[2:], tomllib.loads(sys.stdin.read()))
        elif len(sys.argv) == 4 and sys.argv[1] == "rootfs-type":
            print(rootfs_type(*sys.argv[2:]))
        else:
            raise ValueError(__doc__)
    except (ValueError, OSError, KeyError) as error:
        sys.exit(f"ERROR: {error}")
