#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Kata development lab using Lima sharing or a local/SSH Linux node."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
GUEST_SOURCE = "/home/ubuntu/just-kata/src"
KUBERNETES_FLAVOURS = ("kubeadm", "vanilla", "k0s", "k3s", "rke2", "microk8s")


class LabError(Exception):
    pass


def git_common_dir(source):
    dotgit = source / ".git"
    if dotgit.is_dir():
        return dotgit.resolve()
    if not dotgit.is_file() or not dotgit.read_text().startswith("gitdir: "):
        raise LabError("source must be a Git checkout or worktree")
    gitdir = (source / dotgit.read_text().strip().removeprefix("gitdir: ")).resolve()
    common = gitdir / "commondir"
    path = (gitdir / common.read_text().strip()).resolve() if common.is_file() else gitdir
    if not (gitdir / "HEAD").is_file() or not (path / "objects").is_dir():
        raise LabError("The worktree Git metadata is missing or invalid")
    return path


def read_target(path):
    target = json.loads(Path(path).read_text())
    fields = {
        "name", "backend", "arch", "vm_type", "cpus", "memory_mib", "disk_gib",
    }
    if set(target) - fields:
        raise LabError(f"Unknown target fields: {sorted(set(target) - fields)}")
    if target.get("backend") != "lima":
        raise LabError("backend must be lima")
    if target.get("arch") not in ("amd64", "arm64"):
        raise LabError("arch must describe the execution host: amd64 or arm64")
    required = ["name", "vm_type", "cpus", "memory_mib", "disk_gib"]
    for key in required:
        if not target.get(key):
            raise LabError(f"Missing target field: {key}")
    for key, value in target.items():
        if key in ("cpus", "memory_mib", "disk_gib"):
            if type(value) is not int or value < 1:
                raise LabError(f"{key} must be a positive integer")
        elif not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_./@:-]+", value) or value.startswith("-"):
            raise LabError(f"Invalid value for {key}")
    for key in ("name",):
        if key in target and not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", target[key]):
            raise LabError(f"Invalid {key}")
    if target["vm_type"] not in ("vz", "qemu"):
        raise LabError("vm_type must be vz or qemu")
    return target


class Lab:
    def __init__(self, target, dry_run=False, root=ROOT, source=None, kubernetes="kubeadm", ci_config=None, helm_values=None, release=None, runtime=None, host=None, image=None, kubeconfig=None):
        self.target = target
        self.root = root
        self.dry_run = dry_run
        self.host = host
        self.release = release
        self.requested_image = image
        self.kubeconfig = kubeconfig
        self.pr_image = False
        if host and (target["backend"] != "baremetal" or not re.fullmatch(r"[A-Za-z0-9_@.:-]+", host) or host.startswith("-")):
            raise LabError("host must be an SSH alias or user@node for the baremetal backend")
        self.pins = json.loads((root / "pins.json").read_text())
        self.arch_pins = self.pins["architectures"][target["arch"]]
        if kubernetes not in KUBERNETES_FLAVOURS:
            raise LabError("Unsupported Kubernetes CI provisioner")
        self.kubernetes = kubernetes
        selected_source = source or os.environ.get("JUST_KATA_SOURCE", root.parents[2])
        if host:
            if not Path(selected_source).is_absolute():
                raise LabError("SSH mode requires kata_source to be an absolute path on the node")
            self.source = Path(selected_source)
        else:
            self.source = Path(selected_source).expanduser().resolve()
        self.ci_environment = json.loads((root / "ci/default.json").read_text())
        if ci_config:
            self.ci_environment.update(json.loads(Path(ci_config).read_text()))
        if runtime:
            if not re.fullmatch(r"[a-z][a-z0-9-]*", runtime):
                raise LabError("runtime must be a Kata shim name, for example qemu-nvidia-cpu-runtime-rs")
            self.ci_environment["KATA_HYPERVISOR"] = runtime
        if release:
            if not re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}", release):
                raise LabError("release must be a valid container image tag, for example 4.3.0")
            self.ci_environment.update(DOCKER_REPO="kata-containers/kata-deploy", DOCKER_TAG=release)
        if image:
            if release:
                raise LabError("Select image or release, not both")
            reference, separator, tag = image.rpartition(":")
            registry, slash, repository = reference.partition("/")
            if not separator or not slash or not re.fullmatch(r"[A-Za-z0-9_.:-]+", registry) or \
                    not re.fullmatch(r"[a-z0-9][a-z0-9_./-]*", repository) or \
                    not re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}", tag):
                raise LabError("image must be a fully qualified tagged image, such as ghcr.io/kata-containers/kata-deploy-ci:tag")
            self.ci_environment.update(DOCKER_REGISTRY=registry, DOCKER_REPO=repository, DOCKER_TAG=tag)
        self.helm_values = Path(helm_values) if helm_values else (
            root / "ci/apple-values.yaml" if target.get("vm_type") == "vz" else None)
        for key, value in self.ci_environment.items():
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key) or not isinstance(value, str):
                raise LabError("CI environment must contain variable names with string values")
            if key in ("HOME", "home", "CODEX_HOME", "PATH", "KUBERNETES", "ARCH", "SMOKE_IMAGE"):
                raise LabError(f"Use the controller options instead of overriding {key}")
        self.refresh_identity()

    def refresh_identity(self):
        target = self.target
        self.image = "{DOCKER_REGISTRY}/{DOCKER_REPO}:{DOCKER_TAG}".format(**self.ci_environment)
        self.build_id = hashlib.sha256((self.image + "\0" + str(self.source)).encode()).hexdigest()[:12]
        self.vm_prefix = f"just-kata-{target['name']}-{self.build_id}"
        state_name = f"lima-{target['name']}-{self.build_id}"
        self.state_path = self.root / ".state" / f"{state_name}.json"
        identity = {"target": target, "image": self.image}
        identity.update(source=str(self.source), host=self.host)
        if target["backend"] == "lima":
            identity.update(mount_type="virtiofs", uid=os.getuid())
        self.signature = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()

    def source_command(self, args, **kwargs):
        return self.execute(args, **kwargs) if self.host else self.run(args, **kwargs)

    def select_worktree(self, pr=None, branch=None, create=False):
        if not pr and not branch:
            return
        if pr and branch:
            raise LabError("Select pr or branch, not both")
        if pr and (not str(pr).isdigit() or int(pr) < 1):
            raise LabError("pr must be a positive pull request number")
        if branch and (branch.startswith("-") or not re.fullmatch(r"[A-Za-z0-9_./-]+", branch) or ".." in branch):
            raise LabError("branch must be a local Git branch, remote-tracking ref, or commit")
        label = f"pr-{pr}" if pr else f"branch-{branch}"
        key = hashlib.sha256((str(self.source) + "\0" + label).encode()).hexdigest()[:12]
        if self.host:
            home = "/REMOTE_HOME" if self.dry_run else self.source_command(
                ["sh", "-c", 'printf "%s" "$HOME"'], capture=True).stdout
            cache = Path(home) / ".cache/kata-lab/worktrees"
            exists = False if self.dry_run else self.source_command(
                ["test", "-e", str(cache / key)], check=False).returncode == 0
        else:
            cache = self.root / ".state/worktrees"
            exists = (cache / key).exists()
        checkout = cache / key
        if not exists:
            if not create:
                raise LabError("No worktree prepared for this selection; run up or an install command first")
            self.source_command(["mkdir", "-p", str(cache)])
            git = ["git", "-C", str(self.source)]
            if pr:
                self.source_command([*git, "fetch", "https://github.com/kata-containers/kata-containers.git",
                                     f"refs/pull/{pr}/head"])
                ref = "FETCH_HEAD"
            else:
                ref = f"{branch}^{{commit}}"
            revision = self.source_command([*git, "rev-parse", "--verify", ref], capture=True).stdout.strip()
            self.source_command([*git, "worktree", "add", "--detach", str(checkout), revision or ref])
        self.source = checkout
        revision = "PR_HEAD" if self.dry_run else self.source_command(
            ["git", "-C", str(checkout), "rev-parse", "HEAD"], capture=True).stdout.strip()
        if pr and not self.release and not self.requested_image:
            self.ci_environment.update(DOCKER_REGISTRY="ghcr.io", DOCKER_REPO="kata-containers/kata-deploy-ci",
                                       DOCKER_TAG=f"{pr}-{revision}-{self.target['arch']}", GH_PR_NUMBER=str(pr))
            self.pr_image = True
        self.refresh_identity()
        print(f"Source worktree: {checkout} ({label}, {revision})")

    def run(self, args, *, input=None, capture=False, check=True, timeout=None, env=None, stdin=None):
        print("+ " + shlex.join(str(arg) for arg in args), file=sys.stderr, flush=True)
        if self.dry_run:
            return subprocess.CompletedProcess(args, 0, stdout="", stderr="")
        return subprocess.run(args, input=input, text=True, check=check, timeout=timeout,
                              stdout=subprocess.PIPE if capture else None, env=env, stdin=stdin)

    def lima(self, *args, **kwargs):
        return self.run(["limactl", *args], **kwargs)

    def state(self):
        if not self.state_path.exists():
            if self.dry_run:
                return {"vm": f"{self.vm_prefix}-preview", "signature": self.signature,
                        "owner": "preview"}
            raise LabError("No VM recorded for this target; run just up first")
        state = json.loads(self.state_path.read_text())
        if state["signature"] != self.signature:
            raise LabError("Target or source changed since VM creation; restore its original configuration before teardown")
        return state

    def vm_info(self):
        state = self.state()
        name = state["vm"]
        if self.dry_run:
            return {"name": name, "status": "Stopped", "sshConfigFile": f"/LIMA_HOME/{name}/ssh.config"}
        result = self.lima("list", "--json", name, capture=True, timeout=20)
        instances = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        if len(instances) != 1 or not isinstance(instances[0], dict):
            raise LabError("Recorded Lima VM was not found unambiguously")
        info = instances[0]
        owner = info.get("config", {}).get("param", {}).get("just_kata_owner")
        if info.get("name") != name or not state.get("owner") or owner != state["owner"]:
            raise LabError("Lima VM ownership does not match the workspace record")
        return info

    def connection(self, interactive=False):
        options = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
                   "-o", "StrictHostKeyChecking=accept-new"]
        if not interactive:
            options.append("-T")
        if self.host:
            return options, self.host
        name = self.state()["vm"]
        config = self.vm_info().get("sshConfigFile")
        if not config or not Path(config).is_absolute():
            raise LabError("Lima has not generated an SSH configuration yet")
        options[1:1] = ["-F", config]
        host = f"lima-{name}"
        return options, host

    def execute(self, args, **kwargs):
        if self.target["backend"] == "baremetal" and not self.host:
            return self.run(args, **kwargs)
        options, host = self.connection()
        return self.run([*options, host, shlex.join(args)], **kwargs)

    def guest(self, action, capture=False):
        env = {
            "ARCH": self.target["arch"],
        }
        script = "\n".join(f"export {key}={shlex.quote(value)}" for key, value in env.items())
        script += "\n" + (self.root / "scripts/guest.sh").read_text()
        return self.execute(["sudo", "-n", "bash", "-s", "--", action], input=script, capture=capture)

    def ci_guest(self, action, *arguments, capture=False):
        env = {**self.ci_environment, "ARCH": self.target["arch"], "KUBERNETES": self.kubernetes,
               "SMOKE_IMAGE": self.pins["smoke_image"], "EXECUTION_BACKEND": self.target["backend"]}
        if self.target["backend"] == "baremetal":
            env["JUST_KATA_SOURCE_DIR"] = str(self.source)
        if self.kubeconfig:
            env["KUBECONFIG"] = self.kubeconfig
        if self.helm_values:
            env["JUST_KATA_HELM_VALUES"] = self.helm_values.read_text()
        script = "\n".join(f"export {key}={shlex.quote(value)}" for key, value in env.items())
        script += "\n" + (self.root / "scripts/guest-ci.sh").read_text()
        return self.execute(["bash", "-s", "--", action, *arguments], input=script, capture=capture)

    def doctor(self):
        if self.pr_image:
            if not self.dry_run and not shutil.which("oras"):
                raise LabError("Install oras (or enter nix develop) to verify the PR's CI image before installation")
            try:
                self.run(["oras", "manifest", "fetch", "--descriptor", self.image], capture=True, timeout=60)
            except subprocess.CalledProcessError as error:
                raise LabError(f"PR image unavailable: {self.image}; wait for that architecture's CI build or select image=/release=") from error
        if self.target["backend"] == "baremetal":
            if not self.host and sys.platform != "linux":
                raise LabError("Local baremetal installation requires Linux; use host=user@node for a remote node")
            self.guest("doctor")
            return
        tools = ["ssh", "python3", "limactl"]
        if self.target["vm_type"] == "vz":
            tools += ["swift"]
        missing = [tool for tool in tools if not shutil.which(tool)]
        if missing and not self.dry_run:
            raise LabError("Missing tools: " + ", ".join(missing) + "; install them or enter nix develop")
        self.doctor_lima()

    def doctor_lima(self):
        result = self.lima("--version", capture=True, timeout=20)
        if not self.dry_run:
            match = re.search(r"(?:version\s+)?v?(\d+)\.(\d+)\.(\d+)", result.stdout)
            if not match or tuple(map(int, match.groups())) < (2, 2, 0):
                raise LabError("Lima 2.2.0 or newer is required for the Ubuntu 26.04 profile")
            native = {"arm64": "arm64", "aarch64": "arm64", "x86_64": "amd64"}.get(os.uname().machine)
            if native != self.target["arch"]:
                raise LabError("Nested KVM requires the guest architecture to match the local host")
            if self.target["vm_type"] == "vz" and sys.platform != "darwin":
                raise LabError("The vz driver requires macOS")
            if self.target["vm_type"] == "qemu" and sys.platform != "linux":
                raise LabError("This lab's QEMU profile requires Linux/KVM; use the macos target on macOS")
        if self.target["vm_type"] == "vz":
            if not self.dry_run:
                (self.root / ".state/swift-module-cache").mkdir(parents=True, exist_ok=True)
            self.run(["swift", "-module-cache-path", str(self.root / ".state/swift-module-cache"),
                      str(self.root / "scripts/check-nesting.swift")], timeout=120)
        else:
            # Do not change host modules or forwarding; prove that this user can use KVM.
            self.run(["python3", "-c",
                      "import os,fcntl; f=os.open('/dev/kvm',os.O_RDWR); "
                      "assert fcntl.ioctl(f,0xAE00,0)==12; v=fcntl.ioctl(f,0xAE01,0); "
                      "os.close(v); os.close(f); print('Host KVM is usable')"], timeout=20)

    def render_plan(self, name, owner="preview"):
        arch = {"amd64": "x86_64", "arm64": "aarch64"}[self.target["arch"]]
        image = self.arch_pins["image"]
        config = {
            "minimumLimaVersion": "2.2.0",
            "vmType": self.target["vm_type"], "arch": arch,
            "images": [{"location": image["url"], "arch": arch,
                        "digest": "sha256:" + image["sha256"]}],
            "cpus": self.target["cpus"], "memory": f"{self.target['memory_mib']}MiB",
            "disk": f"{self.target['disk_gib']}GiB", "nestedVirtualization": True,
            "mountType": "virtiofs",
            "mounts": self.source_mounts(),
            "networks": [],
            "containerd": {"system": False, "user": False},
            "ssh": {"loadDotSSHPubKeys": False, "forwardAgent": False},
            "user": {"name": "ubuntu", "uid": os.getuid(), "home": "/home/ubuntu",
                     "shell": "/bin/bash", "passwordlessSudo": True},
            "param": {"just_kata_owner": owner, "internal_netplanOptional": "true"},
            "provision": [{"mode": "system", "script":
                           "#!/bin/sh\nset -eu\ninstall -d /var/lib/just-kata\n"
                           f"install -d -o {os.getuid()} /home/ubuntu/just-kata\n"
                           "echo '{{.Param.just_kata_owner}}' > /var/lib/just-kata/owner\n"
                           "touch /var/lib/cloud/instance/just-kata-booted\n"}],
        }
        # JSON is valid YAML and avoids a controller YAML dependency.
        return json.dumps(config, indent=2) + "\n"

    def create_vm(self):
        if not self.dry_run:
            self.validate_source(self.source)
        name = self.vm_prefix + "-" + ("preview" if self.dry_run else uuid.uuid4().hex[:12])
        owner = "preview" if self.dry_run else uuid.uuid4().hex
        plan = self.render_plan(name, owner)
        plan_path = self.root / ".state" / f"{name}.yaml"
        if self.dry_run:
            print(plan)
        else:
            self.state_path.parent.mkdir(parents=True, exist_ok=True)
            with self.state_path.open("x") as stream:
                json.dump({"vm": name, "signature": self.signature, "owner": owner,
                           "image": self.image, "source": str(self.source)}, stream)
            plan_path.write_text(plan)
        self.lima("create", "--tty=false", f"--name={name}", str(plan_path))

    def start_vm(self):
        info = self.vm_info()
        if info["status"] == "Stopped":
            self.lima("start", "--tty=false", self.state()["vm"], timeout=600)
        elif info["status"] != "Running":
            raise LabError("Lima VM is not stopped or running; inspect it with limactl list")

    @staticmethod
    def verify_checksum(path, expected):
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        if digest.hexdigest() != expected:
            raise LabError(f"Checksum mismatch: {path}; remove the failed download and retry")

    def wait_for_vm(self):
        if self.dry_run:
            print("Wait for VM IP, SSH, and cloud-init (bounded timeout)")
            return
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            try:
                options, host = self.connection()
                self.run([*options, host, "test -f /var/lib/cloud/instance/just-kata-booted"],
                         timeout=20)
                return
            except (LabError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
                time.sleep(3)
        raise LabError("VM did not complete cloud-init within 10 minutes; inspect it with limactl list")

    def up(self):
        if self.target["backend"] != "lima":
            raise LabError("For baremetal use install-kata for an existing cluster or install-cluster for a fresh host")
        self.doctor()
        if not self.state_path.exists():
            self.create_vm()
        self.start_vm()
        self.wait_for_vm()
        self.guest("doctor")
        self.prepare_host()
        self.ci_guest("smoke")

    def prepare_host(self):
        self.prepare_source()
        self.ci_guest("prepare-host")
        self.ci_guest("deploy-k8s")
        self.ci_guest("deploy-kata")

    def install_kata(self):
        if self.target["backend"] != "baremetal":
            raise LabError("install-kata requires backend=baremetal; use up for Lima")
        self.doctor()
        self.prepare_source()
        self.ci_guest("script", "tests/integration/kubernetes/gha-run.sh", "deploy-kata")
        self.ci_guest("smoke")

    def install_cluster(self):
        if self.target["backend"] != "baremetal":
            raise LabError("install-cluster requires backend=baremetal; use up for Lima")
        self.doctor()
        self.prepare_host()
        self.ci_guest("smoke")

    def ci_script(self, script, arguments=()):
        relative = Path(script)
        if relative.is_absolute() or ".." in relative.parts or not relative.parts:
            raise LabError("CI script must be a path relative to the Kata checkout")
        if not self.host:
            path = (self.source / relative).resolve()
            if not path.is_relative_to(self.source) or not path.is_file():
                raise LabError("CI script must exist inside the Kata checkout")
        self.prepare_source()
        self.ci_guest("script", str(relative), *arguments)

    def down(self):
        if self.target["backend"] != "lima":
            raise LabError("down only removes Lima VMs; it does not uninstall a baremetal node")
        name = self.state()["vm"]
        info = self.vm_info()  # Verify the Lima config ownership token before deletion.
        if info["status"] == "Running":
            self.lima("stop", "--tty=false", name, timeout=120)
        elif info["status"] != "Stopped":
            raise LabError("Lima VM state is uncertain; inspect it before deleting")
        self.lima("delete", "--tty=false", name)
        if not self.dry_run:
            self.state_path.unlink()

    @staticmethod
    def validate_source(source):
        path = Path(source).expanduser().resolve()
        if not (path / "src/runtime-rs").is_dir():
            raise LabError("source must be a Kata Containers checkout containing src/runtime-rs")
        git_common_dir(path)
        return path

    def source_mounts(self):
        paths = [self.source]
        if self.source.exists():
            common = git_common_dir(self.source)
            if not common.is_relative_to(self.source):
                paths.append(common)
        return [{"location": str(path), "mountPoint": str(path), "writable": True} for path in paths]

    def prepare_source(self):
        if not self.host and not self.dry_run:
            self.validate_source(self.source)
        source = shlex.quote(str(self.source))
        command = f"test -f {source}/tests/integration/kubernetes/gha-run.sh"
        if self.target["backend"] == "lima":
            link = shlex.quote(GUEST_SOURCE)
            command = (f"findmnt -rn -M {source} -t virtiofs >/dev/null\ntest -w {source}\n"
                       f"{command}\ngit -C {source} rev-parse --git-dir >/dev/null\n"
                       "mkdir -p /home/ubuntu/just-kata\n"
                       f"if test -e {link} && ! test -L {link}; then "
                       "echo 'Checkout path is occupied; use a fresh VM' >&2; exit 1; fi\n"
                       f"ln -sfn {source} {link}")
        self.execute(["sh", "-ec", command])
        print(f"Using checkout: {self.source}")

    def load_image(self):
        """Fetch the pinned workload on the controller, then stream an OCI import."""
        if not self.dry_run and not shutil.which("oras"):
            raise LabError("load-image requires oras on the controller; install it or enter nix develop")
        layout = self.root / ".state" / "smoke-oci"
        archive = self.root / ".state" / "smoke-oci.tar"
        if not self.dry_run:
            layout.parent.mkdir(parents=True, exist_ok=True)
        self.run(["oras", "cp", "--to-oci-layout", self.pins["smoke_image"], f"{layout}:smoke"], timeout=180)
        if not self.dry_run:
            expected = self.pins["smoke_image"].split("@", 1)[1]
            index = json.loads((layout / "index.json").read_text())
            matches = [item for item in index["manifests"] if item["digest"] == expected]
            if not matches:
                raise LabError("OCI layout does not contain the pinned smoke image")
            for blob in (layout / "blobs/sha256").iterdir():
                self.verify_checksum(blob, blob.name)
            with tarfile.open(archive, "w") as stream:
                for name in ("oci-layout", "blobs"):
                    stream.add(layout / name, arcname=name)
                # Name only the pinned top-level image. Naming every digest
                # would also ask ctr to unpack non-runnable attestations.
                descriptor = {**matches[0], "annotations": {
                    "org.opencontainers.image.ref.name": self.pins["smoke_image"],
                }}
                index_bytes = json.dumps({"schemaVersion": 2, "manifests": [descriptor]}).encode()
                entry = tarfile.TarInfo("index.json")
                entry.size = len(index_bytes)
                stream.addfile(entry, io.BytesIO(index_bytes))
        command = [
            "sudo", "-n", *self.ctr_command(), "--namespace", "k8s.io",
            "images", "import", "--local", "--platform", f"linux/{self.target['arch']}",
            "--base-name", self.pins["smoke_image"].split("@", 1)[0], "-",
        ]
        if self.dry_run:
            self.execute(command)
        else:
            with archive.open("rb") as stream:
                self.execute(command, stdin=stream, timeout=180)

    def ctr_command(self):
        commands = {
            "kubeadm": ["ctr", "--address", "/run/containerd/containerd.sock"],
            "vanilla": ["ctr", "--address", "/run/containerd/containerd.sock"],
            "k3s": ["k3s", "ctr"],
            "rke2": ["/var/lib/rancher/rke2/bin/ctr", "--address", "/run/k3s/containerd/containerd.sock"],
            "k0s": ["k0s", "ctr"],
            "microk8s": ["microk8s", "ctr"],
        }
        return commands[self.kubernetes]

    def logs(self):
        result = self.ci_guest("logs", capture=True)
        destination = self.root / "results" / f"{self.target['name']}-{self.build_id}-{time.time_ns()}"
        if not self.dry_run:
            destination.mkdir(parents=True)
            (destination / "host.log").write_text(result.stdout)
            (destination / "pins.json").write_text(json.dumps(self.pins, indent=2) + "\n")
            (destination / "target.json").write_text(json.dumps(self.target, indent=2) + "\n")
            (destination / "ci.json").write_text(json.dumps({"kubernetes": self.kubernetes,
                                                           "source": str(self.source), "image": self.image}, indent=2) + "\n")
        print(f"Diagnostics: {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    default_target = "macos" if sys.platform == "darwin" else "local"
    parser.add_argument("--target", default=os.environ.get("JUST_KATA_TARGET", str(ROOT / f"targets/{default_target}.json")))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--source", default=os.environ.get("JUST_KATA_SOURCE"))
    parser.add_argument("--kubernetes", choices=KUBERNETES_FLAVOURS, default=os.environ.get("JUST_KATA_KUBERNETES", "kubeadm"))
    parser.add_argument("--ci-config", default=os.environ.get("JUST_KATA_CI_CONFIG"))
    parser.add_argument("--helm-values", default=os.environ.get("JUST_KATA_HELM_VALUES"))
    parser.add_argument("--release", default=os.environ.get("JUST_KATA_RELEASE"),
                        help="test a published release in a separate VM instead of the latest CI build")
    parser.add_argument("--runtime", default=os.environ.get("JUST_KATA_RUNTIME"),
                        help="Kata shim name, for example qemu-runtime-rs or qemu-nvidia-cpu-runtime-rs")
    parser.add_argument("--backend", choices=["lima", "baremetal"], default=os.environ.get("JUST_KATA_BACKEND", "lima"))
    parser.add_argument("--host", default=os.environ.get("JUST_KATA_HOST"), help="SSH alias or user@node for baremetal")
    parser.add_argument("--arch", choices=["amd64", "arm64"], default=os.environ.get("JUST_KATA_ARCH"))
    parser.add_argument("--pr", default=os.environ.get("JUST_KATA_PR"), help="Kata pull request number")
    parser.add_argument("--branch", default=os.environ.get("JUST_KATA_BRANCH"), help="Git branch, remote-tracking ref, or commit")
    parser.add_argument("--image", default=os.environ.get("JUST_KATA_IMAGE"), help="fully qualified tagged kata-deploy image")
    parser.add_argument("--kubeconfig", default=os.environ.get("JUST_KATA_KUBECONFIG"), help="kubeconfig path on the execution node")
    parser.add_argument("action", choices=["doctor", "up", "install-kata", "install-cluster", "test", "ssh", "logs", "down", "load-image", "probe", "ci", "ci-script"])
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        if args.backend == "baremetal":
            arch = args.arch or {"x86_64": "amd64", "aarch64": "arm64", "arm64": "arm64"}.get(os.uname().machine)
            if args.host and not args.arch and not args.dry_run:
                if not re.fullmatch(r"[A-Za-z0-9_@.:-]+", args.host) or args.host.startswith("-"):
                    raise LabError("Invalid SSH host")
                machine = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-T",
                                          args.host, "uname -m"], check=True, capture_output=True, text=True).stdout.strip()
                arch = {"x86_64": "amd64", "aarch64": "arm64"}.get(machine)
            if not arch:
                raise LabError("Only amd64 and arm64 execution hosts are supported")
            target = {"name": "baremetal", "backend": "baremetal", "arch": arch}
        else:
            target = read_target(args.target)
        lab = Lab(target, args.dry_run, source=args.source,
                  kubernetes=args.kubernetes, ci_config=args.ci_config, helm_values=args.helm_values,
                  release=args.release, runtime=args.runtime, host=args.host or None, image=args.image or None,
                  kubeconfig=args.kubeconfig or None)
        if args.backend == "baremetal" and not args.host and sys.platform != "linux" and not args.dry_run:
            raise LabError("Local baremetal commands require Linux; select host=user@node to run remotely")
        lab.select_worktree(args.pr or None, args.branch or None,
                            create=args.action in ("up", "install-kata", "install-cluster"))
        argument = args.arguments[0] if args.arguments else None
        print(f"Target: {lab.target['name']} ({lab.target['backend']}, {lab.target['arch']}), Kubernetes: {lab.kubernetes}", flush=True)
        print(f"Kata image: {lab.image}", flush=True)
        print(f"Runtime: {lab.ci_environment['KATA_HYPERVISOR']}", flush=True)
        print(f"Source: {lab.source}", flush=True)
        if args.action == "test":
            if args.arguments not in ([], ["smoke"]):
                raise LabError("Use ci run-tests for upstream suites")
            lab.ci_guest("smoke")
        elif args.action == "probe":
            lab.guest("probe")
        elif args.action == "ssh":
            if lab.target["backend"] == "baremetal" and not lab.host:
                raise LabError("Already running on the baremetal node; use the current shell")
            options, host = lab.connection(interactive=True)
            lab.run([*options, host])
        elif args.action in ("ci", "ci-script"):
            if not argument:
                raise LabError(f"{args.action} requires an upstream action or script path")
            script = "tests/integration/kubernetes/gha-run.sh" if args.action == "ci" else argument
            arguments = args.arguments if args.action == "ci" else args.arguments[1:]
            lab.ci_script(script, arguments)
        else:
            if args.arguments:
                raise LabError(f"Unexpected argument for {args.action}")
            getattr(lab, args.action.replace("-", "_"))()
    except (LabError, OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
