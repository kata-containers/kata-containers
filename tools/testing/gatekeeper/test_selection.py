#!/usr/bin/env python3
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

"""Regression tests for suite selection and its workflow/Gatekeeper contracts."""

from contextlib import redirect_stdout
from copy import deepcopy
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import selection
from skips import Checks


K8S = "tests/integration/kubernetes/"


class SelectionTests(unittest.TestCase):
    def select(self, paths, **kwargs):
        return selection.select(paths, **kwargs)

    def test_test_directories(self):
        cases = {
            "tests/integration/nerdctl/gha-run.sh": {"nerdctl"},
            "tests/integration/docker/docker.bats": {"docker"},
            "tests/integration/nydus/nydus_tests.sh": {"nydus"},
            "tests/integration/cri-containerd/gha-run.sh": {"cri", "sandboxapi"},
            "tests/stability/gha-run.sh": {"stability"},
            "tests/functional/kata-agent-apis/api-tests/test.bats": {"agent-api"},
            "tests/functional/kata-monitor/gha-run.sh": {"monitor"},
            "tests/functional/kata-deploy/gha-run.sh": {"deploy", "monitor"},
            "tests/integration/substrate/setup.sh": {"substrate"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(self.select([path]).suites, expected)

    def test_core_and_unknown_paths_run_everything(self):
        for path in ["src/runtime/virtcontainers/sandbox.go", "src/runtime-rs/src/lib.rs",
                     "src/agent/src/main.rs", "src/libs/kata-types/src/lib.rs",
                     "src/tools/kata-ctl/src/main.rs", "src/tools/agent-ctl/src/main.rs",
                     "tools/osbuilder/rootfs-builder/rootfs.sh", "tools/packaging/kernel/config",
                     "tests/common.bash", "tests/gha-run-k8s-common.sh", "Cargo.lock",
                     ".github/workflows/ci.yaml", "new-component/source.rs"]:
            with self.subTest(path=path):
                result = self.select(["tests/integration/nerdctl/gha-run.sh", path])
                self.assertEqual(result.suites, selection.SUITES)
                self.assertTrue(result.build)

    def test_mixed_suites_are_unioned(self):
        result = self.select(["tests/integration/nerdctl/gha-run.sh",
                              "tests/integration/docker/gha-run.sh", "docs/testing.md"])
        self.assertEqual(result.suites, {"nerdctl", "docker"})

    def test_k8s_changes_select_whole_suites(self):
        for path in ["k8s-memory.bats", "k8s-exec.bats", "lib.sh",
                     "runtimeclass_workloads/pod-memory.yaml", "k8s_bats_runner.sh"]:
            with self.subTest(path=path):
                result = self.select([K8S + path])
                self.assertEqual(result.suites, {"kubernetes", "tee", "gpu"})

    def test_genpolicy_selects_related_suites(self):
        result = self.select(["src/tools/genpolicy/src/main.rs"])
        self.assertEqual(result.suites, {"kubernetes", "tee", "gpu"})

    def test_guest_dependencies_select_tee_including_coco_and_gpu(self):
        for dependencies in [{"coco-guest-components"}, {"coco-trustee"},
                             {"coco-guest-components", "coco-trustee"}]:
            result = self.select(["versions.yaml"], dependency_changes=dependencies)
            self.assertEqual(result.suites, {"tee"})
            self.assertIn("ci/run-kata-coco-tests", result.jobs)
            self.assertIn("ci/run-k8s-tests-on-nvidia-gpu", result.jobs)
            self.assertNotIn("ci/run-k8s-tests-on-free-runner", result.jobs)

    def test_other_dependency_changes_run_everything(self):
        for dependencies in [None, set(), {"qemu"}, {"coco-trustee", "containerd"}]:
            result = self.select(["versions.yaml"], dependency_changes=dependencies)
            self.assertEqual(result.suites, selection.SUITES)

    def test_untested_component_builds_without_integration_tests(self):
        result = self.select(["src/tools/log-parser/main.go"])
        self.assertEqual(result.suites, set())
        self.assertEqual(result.jobs, [])
        self.assertTrue(result.build)

    def test_documentation_does_not_require_artifacts(self):
        result = self.select(["docs/testing.md", "README.md"])
        self.assertEqual(result.suites, set())
        self.assertFalse(result.build)

    def test_config_rejects_unknown_suite(self):
        config = deepcopy(selection.CONFIG)
        config["paths"][0]["suites"] = ["typo"]
        with self.assertRaises(ValueError):
            selection.validate_config(config)

    def test_gatekeeper_and_selector_share_the_same_mapping(self):
        self.assertIs(Checks().mapping, selection.CONFIG["mapping"])


    def test_untested_component_does_not_schedule_integration_jobs(self):
        result = self.select(["src/tools/log-parser/main.go"])
        with patch("skips.from_git", return_value=result):
            output = io.StringIO()
            with redirect_stdout(output):
                Checks().run(False, "main")
        values = dict(line.split("=", 1) for line in output.getvalue().splitlines())
        self.assertEqual(values["skip_test"], "yes")
        self.assertEqual(values["skip_build"], "no")
        self.assertEqual(json.loads(values["test_jobs"]), [])

    def test_skipper_outputs_and_required_jobs_agree(self):
        result = self.select(["tests/integration/nydus/gha-run.sh"])
        with patch("skips.from_git", return_value=result):
            output = io.StringIO()
            with redirect_stdout(output):
                Checks().run(False, "main")
            values = dict(line.split("=", 1) for line in output.getvalue().splitlines())
            self.assertEqual(values["skip_test"], "no")
            self.assertEqual(values["skip_build"], "no")
            self.assertEqual(json.loads(values["test_jobs"]), result.jobs)
            output = io.StringIO()
            with redirect_stdout(output):
                Checks().run(True, "main")
            names = output.getvalue().splitlines()[0]
            self.assertIn("run-nydus", names)
            self.assertNotIn("run-containerd-stability", names)
            self.assertNotIn("run-k8s-tests", names)


class GitSelectionTests(unittest.TestCase):
    def setUp(self):
        self.tempdir = tempfile.TemporaryDirectory()
        self.addCleanup(self.tempdir.cleanup)
        self.repo = Path(self.tempdir.name)
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Selection test")
        self.git("config", "user.email", "selection@example.invalid")
        self.write("versions.yaml", "externals:\n  coco-trustee:\n    version: old\n  qemu:\n    version: old\n")
        self.commit()
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("checkout", "-b", "topic")
        self.addCleanup(patch.stopall)
        patch("selection.git", side_effect=self.git).start()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], stderr=subprocess.DEVNULL)

    def write(self, name, content="fixture\n"):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-m", "test: selection fixture")

    def test_upstream_changes_and_dirty_worktree_are_excluded(self):
        self.git("checkout", "main")
        self.write("src/agent/upstream.rs")
        self.commit()
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("checkout", "topic")
        self.write("tests/integration/nerdctl/gha-run.sh")
        self.commit()
        self.write("src/agent/local.rs")
        self.assertEqual(selection.from_git("main").suites, {"nerdctl"})

    def test_rename_across_suites_includes_both_sides(self):
        self.write("tests/integration/nerdctl/fixture.sh")
        self.commit()
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")
        self.git("mv", "tests/integration/nerdctl", "tests/integration/docker")
        self.commit()
        self.assertEqual(selection.from_git("main").suites, {"nerdctl", "docker"})

    def test_versions_yaml_semantic_dependency_diff(self):
        self.write("versions.yaml", "# comment\nexternals:\n  coco-trustee:\n    version: new\n  qemu:\n    version: old\n")
        self.commit()
        self.assertEqual(selection.from_git("main").suites, {"tee"})
        self.write("versions.yaml", "externals:\n  coco-trustee:\n    version: new\n  qemu:\n    version: new\n")
        self.commit()
        self.assertEqual(selection.from_git("main").suites, selection.SUITES)


if __name__ == "__main__":
    unittest.main()
