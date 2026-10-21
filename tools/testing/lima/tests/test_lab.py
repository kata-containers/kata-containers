# SPDX-License-Identifier: Apache-2.0

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("lab", ROOT / "scripts/lab.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class LabTests(unittest.TestCase):
    def repository(self, path):
        path.mkdir()
        (path / "src/runtime-rs").mkdir(parents=True)
        (path / "src/runtime-rs/marker").write_text("committed\n")
        script = path / "tests/integration/kubernetes/gha-run.sh"
        script.parent.mkdir(parents=True)
        script.write_text("#!/bin/sh\n")
        subprocess.run(["git", "init", "-q", "-b", "main", str(path)], check=True)
        subprocess.run(["git", "-C", str(path), "add", "."], check=True)
        subprocess.run(["git", "-C", str(path), "-c", "user.name=Test", "-c", "user.email=test@example.com",
                        "commit", "-qm", "Initial fixture"], check=True)
        return path

    def lab(self, name="local", dry_run=False):
        lab = module.Lab(module.read_target(ROOT / f"targets/{name}.json"), dry_run)
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        lab.state_path = Path(directory.name) / lab.state_path.name
        return lab

    def test_failed_smoke_propagates(self):
        lab = self.lab()
        with patch.object(lab, "doctor"), patch.object(lab, "create_vm"), patch.object(lab, "start_vm"), \
                patch.object(lab, "wait_for_vm"), patch.object(lab, "guest"), patch.object(lab, "prepare_host"), \
                patch.object(lab, "ci_guest", side_effect=subprocess.CalledProcessError(1, "smoke")):
            with self.assertRaises(subprocess.CalledProcessError):
                lab.up()

    def test_vm_up_uses_upstream_ci_before_a_kubernetes_pod(self):
        lab = self.lab("macos")
        with patch.object(lab, "doctor"), patch.object(lab, "create_vm"), patch.object(lab, "start_vm"), patch.object(lab, "wait_for_vm"), \
                patch.object(lab, "prepare_source") as shared, patch.object(lab, "ci_guest") as ci, \
                patch.object(lab, "guest") as legacy:
            lab.up()
        shared.assert_called_once_with()
        self.assertEqual([call.args[0] for call in ci.call_args_list],
                         ["prepare-host", "deploy-k8s", "deploy-kata", "smoke"])
        legacy.assert_called_once_with("doctor")

    def test_all_ci_provisioners_are_forwarded_without_shell_expansion(self):
        import shlex
        for flavour in module.KUBERNETES_FLAVOURS:
            with self.subTest(flavour=flavour):
                lab = module.Lab(module.read_target(ROOT / "targets/local.json"), kubernetes=flavour, dry_run=True)
                with patch.object(lab, "run") as run:
                    lab.ci_guest("script", "ci/example.sh", "value; still one argument")
                self.assertIn(f"export KUBERNETES={flavour}\n", run.call_args.kwargs["input"])
                self.assertEqual(shlex.split(run.call_args.args[0][-1]),
                                 ["bash", "-s", "--", "script", "ci/example.sh", "value; still one argument"])

    def test_ci_script_cannot_escape_the_checkout(self):
        lab = self.lab()
        with tempfile.TemporaryDirectory() as directory:
            lab.source = Path(directory)
            (lab.source / "escape.sh").symlink_to(ROOT / "scripts/guest.sh")
            for name in ["../elsewhere.sh", "/tmp/elsewhere.sh", "escape.sh"]:
                with self.subTest(name=name), patch.object(lab, "prepare_source") as shared:
                    with self.assertRaises(module.LabError):
                        lab.ci_script(name)
                    shared.assert_not_called()

    def test_cluster_distribution_change_requires_a_fresh_target(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "prepared").touch()
            (path / "cluster-distribution").write_text("kubeadm\n")
            script = r'''
ARCH=arm64 KUBERNETES=k3s SMOKE_IMAGE=test
source scripts/guest-ci.sh library
PREPARED_MARKER=$1/prepared
CLUSTER_MARKER=$1/cluster-distribution
require_prepared
'''
            result = subprocess.run(["bash", "-c", script, "test", directory], cwd=ROOT, capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("use a fresh VM target", result.stderr)

    def test_chart_overrides_follow_ci_values_without_affecting_helm_version(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "src/tests/integration/kubernetes").mkdir(parents=True)
            (path / "src/tests/integration/kubernetes/gha-run.sh").touch()
            (path / "src/tests/gha-run-k8s-common.sh").write_text('ensure_helm() { :; }\n')
            (path / "bin").mkdir()
            (path / "bin/helm").write_text('#!/bin/bash\nprintf "%s\\n" "$@" >> "$HELM_TRACE"\n')
            (path / "bin/helm").chmod(0o755)
            (path / "installed").mkdir()
            (path / "installed/helm").write_text('#!/bin/bash\necho "Host Helm was used" >&2\nexit 42\n')
            (path / "installed/helm").chmod(0o755)
            script = r'''
ARCH=arm64 KUBERNETES=kubeadm SMOKE_IMAGE=test
source scripts/guest-ci.sh library
export PATH="$1/installed:$PATH"
export PATH="$1/bin:$PATH"
LAB_CI_DIR=$1
CI_SOURCE=$1/src
JUST_KATA_HELM_VALUES='shims: {}'
enable_helm_overrides
bash -c 'helm template chart --values ci.yaml; helm version'
'''
            result = subprocess.run(["bash", "-c", script, "test", directory], cwd=ROOT,
                                    env={**os.environ, "PATH": str(path / "bin") + ":" + os.environ["PATH"],
                                         "HELM_TRACE": str(path / "trace")}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((path / "trace").read_text().splitlines(),
                             ["template", "chart", "--values", "ci.yaml", "--values",
                              str(path / "helm-overrides.yaml"), "version"])

    def test_dry_run_never_spawns_commands_or_creates_state(self):
        lab = self.lab("local", dry_run=True)
        with tempfile.TemporaryDirectory() as directory:
            lab.state_path = Path(directory) / "state.json"
            with patch("subprocess.run") as run, contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                lab.up()
            run.assert_not_called()
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_down_requires_matching_lima_ownership(self):
        lab = self.lab("local")
        info = {"name": "lab-owned", "config": {"param": {"just_kata_owner": "other"}}}
        with patch.object(lab, "state", return_value={"vm": "lab-owned", "owner": "mine"}), \
                patch.object(lab, "lima", return_value=subprocess.CompletedProcess([], 0, json.dumps(info))) as lima:
            with self.assertRaises(module.LabError):
                lab.down()
        self.assertEqual(lima.call_count, 1)
        self.assertEqual(lima.call_args.args[0], "list")

    def test_down_failure_preserves_ownership_record(self):
        lab = self.lab("local")
        with tempfile.TemporaryDirectory() as directory:
            lab.state_path = Path(directory) / "state.json"
            lab.state_path.write_text(json.dumps({"vm": "lab-owned", "signature": lab.signature, "owner": "mine"}))
            with patch.object(lab, "vm_info", return_value={"status": "Stopped"}), \
                    patch.object(lab, "lima", side_effect=subprocess.CalledProcessError(1, "delete")):
                with self.assertRaises(subprocess.CalledProcessError):
                    lab.down()
            self.assertTrue(lab.state_path.exists())

    def test_changed_target_cannot_delete_previous_vm(self):
        lab = self.lab("local")
        with tempfile.TemporaryDirectory() as directory:
            lab.state_path = Path(directory) / "state.json"
            lab.state_path.write_text('{"vm":"old-vm","signature":"different"}')
            with patch.object(lab, "lima") as lima:
                with self.assertRaises(module.LabError):
                    lab.down()
            lima.assert_not_called()

    def test_default_source_is_the_containing_kata_checkout(self):
        with patch.dict(os.environ, {}, clear=True):
            lab = self.lab()
        self.assertEqual(lab.source, ROOT.parents[2])
        self.assertTrue((lab.source / "tests/integration/kubernetes/gha-run.sh").is_file())

    def test_shared_source_is_checked_without_copying_files(self):
        lab = self.lab(dry_run=True)
        with patch.object(lab, "run") as run, contextlib.redirect_stdout(io.StringIO()):
            lab.prepare_source()
        run.assert_called_once()
        args = run.call_args.args[0]
        self.assertEqual(args[0], "ssh")
        self.assertIn(f"findmnt -rn -M {lab.source} -t virtiofs", args[-1])
        self.assertIn(f"test -w {lab.source}", args[-1])




    def test_baremetal_existing_cluster_installs_only_kata(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"})
        with patch.object(lab, "doctor"), patch.object(lab, "prepare_source"), patch.object(lab, "ci_guest") as ci:
            lab.install_kata()
        self.assertEqual([call.args for call in ci.call_args_list],
                         [("script", "tests/integration/kubernetes/gha-run.sh", "deploy-kata"), ("smoke",)])

    def test_baremetal_fresh_host_installs_both_stages(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"})
        with patch.object(lab, "doctor"), patch.object(lab, "prepare_source"), patch.object(lab, "ci_guest") as ci:
            lab.install_cluster()
        self.assertEqual([call.args[0] for call in ci.call_args_list],
                         ["prepare-host", "deploy-k8s", "deploy-kata", "smoke"])

    def test_baremetal_ssh_uses_the_remote_checkout_without_copying(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"},
                         host="user@node", source="/srv/kata", dry_run=True)
        with patch.object(lab, "run") as run:
            lab.ci_guest("script", "ci/example.sh", "literal; argument")
        args = run.call_args.args[0]
        self.assertEqual(args[0], "ssh")
        self.assertIn("user@node", args)
        self.assertIn("export JUST_KATA_SOURCE_DIR=/srv/kata\n", run.call_args.kwargs["input"])
        import shlex
        self.assertEqual(shlex.split(args[-1]), ["bash", "-s", "--", "script", "ci/example.sh", "literal; argument"])

    def test_baremetal_local_executes_directly_without_ssh(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"}, dry_run=True)
        with patch.object(lab, "run") as run:
            lab.ci_guest("smoke")
        self.assertEqual(run.call_args.args[0], ["bash", "-s", "--", "smoke"])

    def test_baremetal_does_not_create_or_delete_vms(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"})
        with patch.object(lab, "lima") as lima:
            for action in (lab.up, lab.down):
                with self.assertRaises(module.LabError):
                    action()
        lima.assert_not_called()

    def test_another_source_cannot_reuse_an_existing_mount_record(self):
        lab = self.lab()
        with tempfile.TemporaryDirectory() as directory:
            other = module.Lab(lab.target, source=directory)
            other.state_path = lab.state_path
            lab.state_path.write_text(json.dumps({"vm": "existing", "signature": lab.signature, "owner": "mine"}))
            with patch.object(other, "lima") as lima:
                with self.assertRaisesRegex(module.LabError, "source changed"):
                    other.down()
            lima.assert_not_called()

    def test_broken_worktree_metadata_is_rejected_before_vm_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "src/runtime-rs").mkdir(parents=True)
            (source / ".git").write_text("gitdir: /host-only/git/path\n")
            lab = module.Lab(module.read_target(ROOT / "targets/local.json"), source=source)
            with patch.object(lab, "lima") as lima:
                with self.assertRaisesRegex(module.LabError, "metadata"):
                    lab.create_vm()
            lima.assert_not_called()

    def test_reject_target_injection_and_unknown_fields(self):
        original = module.read_target(ROOT / "targets/local.json")
        for key, value in [("name", "../../other"), ("network", 'net"\ncmds: []'),
                           ("ssh_host", "-oProxyCommand=bad"), ("cpus", True), ("typo", "value")]:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "target.json"
                path.write_text(json.dumps({**original, key: value}))
                with self.assertRaises(module.LabError):
                    module.read_target(path)

    def test_lima_plan_pins_image_and_enables_nesting_without_vmnet(self):
        for target, driver, arch in [("macos", "vz", "aarch64"), ("local", "qemu", "x86_64"),
                                   ("linux-arm64", "qemu", "aarch64")]:
            with self.subTest(target=target):
                lab = self.lab(target, dry_run=True)
                plan = json.loads(lab.render_plan("just-kata-test", "owner-token"))
                self.assertEqual(plan["vmType"], driver)
                self.assertEqual(plan["arch"], arch)
                self.assertTrue(plan["nestedVirtualization"])
                self.assertEqual(plan["disk"], "50GiB")
                self.assertEqual(plan["networks"], [])
                self.assertEqual(plan["mountType"], "virtiofs")
                self.assertEqual(plan["mounts"], [{"location": str(lab.source),
                                                      "mountPoint": str(lab.source), "writable": True}])
                self.assertEqual(plan["user"]["uid"], os.getuid())
                self.assertEqual(plan["containerd"], {"user": False, "system": False})
                self.assertEqual(plan["images"][0]["digest"], "sha256:" + lab.arch_pins["image"]["sha256"])
                self.assertEqual(plan["param"]["just_kata_owner"], "owner-token")

    def test_lima_dry_run_has_no_effects(self):
        for target in ("macos", "local"):
            lab = self.lab(target, dry_run=True)
            with tempfile.TemporaryDirectory() as directory:
                lab.state_path = Path(directory) / "state.json"
                with patch("subprocess.run") as run, patch.object(Path, "write_text") as write, \
                        patch.object(Path, "mkdir") as mkdir, contextlib.redirect_stdout(io.StringIO()), \
                        contextlib.redirect_stderr(io.StringIO()):
                    lab.up()
                run.assert_not_called()
                write.assert_not_called()
                mkdir.assert_not_called()

    def test_lima_connection_uses_managed_ssh_config_not_guest_ip(self):
        lab = self.lab("macos", dry_run=True)
        options, host = lab.connection()
        self.assertIn("-F", options)
        self.assertIn(f"/LIMA_HOME/{lab.vm_prefix}-preview/ssh.config", options)
        self.assertEqual(host, f"lima-{lab.vm_prefix}-preview")

    def test_lima_uses_new_state_namespace(self):
        lab = self.lab("macos")
        self.assertEqual(lab.state_path.name, f"lima-macos-{lab.build_id}.json")
        self.assertNotEqual(lab.state_path.name, "macos.json")

    def test_default_uses_the_upstream_latest_ci_image(self):
        lab = self.lab("macos")
        self.assertEqual(lab.image, "quay.io/kata-containers/kata-deploy-ci:kata-containers-latest")




    def test_releases_use_separate_owned_vms_and_teardown_records(self):
        target = module.read_target(ROOT / "targets/macos.json")
        latest = module.Lab(target)
        release = module.Lab(target, release="4.3.0")
        same_release = module.Lab(target, release="4.3.0")
        other_release = module.Lab(target, release="4.2.0")
        self.assertEqual(release.state_path, same_release.state_path)
        self.assertEqual(len({latest.state_path, release.state_path, other_release.state_path}), 3)
        with tempfile.TemporaryDirectory() as directory:
            for lab in (latest, release):
                lab.root = Path(directory)
                lab.state_path = lab.root / ".state" / lab.state_path.name
                with patch.object(lab, "lima"):
                    lab.create_vm()
            latest_state = latest.state()
            release_state = release.state()
            self.assertNotEqual(latest_state["vm"], release_state["vm"])
            self.assertNotEqual(latest_state["owner"], release_state["owner"])
            self.assertEqual(release_state["image"], release.image)
            with patch.object(release, "vm_info", return_value={"status": "Stopped"}), \
                    patch.object(release, "lima") as lima:
                release.down()
            lima.assert_called_once_with("delete", "--tty=false", release_state["vm"])
            self.assertEqual(latest.state(), latest_state)
            self.assertFalse(release.state_path.exists())

    def test_release_rejects_invalid_tags_before_running_commands(self):
        for release in ("../../wrong", "4.3.0; touch unwanted", "-wrong"):
            with self.subTest(release=release), patch("subprocess.run") as run:
                with self.assertRaisesRegex(module.LabError, "container image tag"):
                    module.Lab(module.read_target(ROOT / "targets/macos.json"), release=release)
                run.assert_not_called()

    def test_create_failure_retains_record_for_recovery(self):
        lab = self.lab("macos")
        with tempfile.TemporaryDirectory() as directory:
            lab.root = Path(directory)
            lab.state_path = lab.root / ".state/lima-macos.json"
            with patch.object(lab, "lima", side_effect=subprocess.CalledProcessError(1, "create")):
                with self.assertRaises(subprocess.CalledProcessError):
                    lab.create_vm()
            state = json.loads(lab.state_path.read_text())
            self.assertTrue(state["owner"])
            plan = json.loads((lab.root / ".state" / (state["vm"] + ".yaml")).read_text())
            self.assertEqual(plan["param"]["just_kata_owner"], state["owner"])

    def test_lima_rejects_missing_ownership_even_if_name_matches(self):
        lab = self.lab("macos")
        with patch.object(lab, "state", return_value={"vm": "same-name", "owner": "mine"}), \
                patch.object(lab, "lima", return_value=subprocess.CompletedProcess([], 0, '{"name":"same-name","config":{}}')):
            with self.assertRaises(module.LabError):
                lab.vm_info()

    def test_stopped_owned_vm_is_restarted(self):
        lab = self.lab("macos")
        with patch.object(lab, "vm_info", return_value={"status": "Stopped"}), \
                patch.object(lab, "state", return_value={"vm": "owned"}), patch.object(lab, "lima") as lima:
            lab.start_vm()
        self.assertEqual(lima.call_args.args, ("start", "--tty=false", "owned"))

    def test_old_lima_version_is_rejected_before_hardware_checks(self):
        lab = self.lab("macos")
        with patch.object(lab, "lima", return_value=subprocess.CompletedProcess([], 0, "limactl version 2.1.0")), \
                patch.object(lab, "run") as run:
            with self.assertRaisesRegex(module.LabError, "2.2.0"):
                lab.doctor_lima()
        run.assert_not_called()

    def test_checksum_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "image"
            path.write_bytes(b"corrupt image")
            with self.assertRaises(module.LabError):
                module.Lab.verify_checksum(path, "0" * 64)

    def test_load_image_on_local_baremetal_imports_without_ssh(self):
        lab = module.Lab({"name": "baremetal", "backend": "baremetal", "arch": "amd64"}, dry_run=True)
        with patch.object(lab, "run") as run, patch.object(lab, "connection") as connection:
            lab.load_image()
        connection.assert_not_called()
        command = run.call_args.args[0]
        self.assertEqual(command[:3], ["sudo", "-n", "ctr"])
        self.assertIn("import", command)
        self.assertEqual(command[-1], "-")

    def test_load_image_rejects_an_unpinned_layout_before_ssh(self):
        lab = self.lab("macos")
        with tempfile.TemporaryDirectory() as directory:
            lab.root = Path(directory)
            layout = lab.root / ".state/smoke-oci"
            layout.mkdir(parents=True)
            (layout / "index.json").write_text('{"manifests":[{"digest":"sha256:wrong"}]}')
            with patch.object(module.shutil, "which", return_value="oras"), \
                    patch.object(lab, "run"), patch.object(lab, "connection") as connection:
                with self.assertRaises(module.LabError):
                    lab.load_image()
                connection.assert_not_called()


if __name__ == "__main__":
    unittest.main()
