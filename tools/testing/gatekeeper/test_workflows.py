#!/usr/bin/env python3
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0

"""Contracts between suite selection, workflow scheduling, and Gatekeeper."""

import json
from pathlib import Path
import unittest

import yaml

import selection
from skips import Checks


ROOT = Path(__file__).resolve().parents[3]


class WorkflowSelectionTests(unittest.TestCase):
    def test_workflow_and_gatekeeper_mapping_covers_every_suite(self):
        mapping = Checks().mapping
        self.assertEqual(set(mapping), selection.SUITES | {"static", "build"})
        for suite in selection.SUITES:
            jobs = selection.CONFIG["mapping"][suite]["jobs"]
            for job in jobs:
                with self.subTest(suite=suite, job=job):
                    workflow, job_id = job.split("/")
                    with (ROOT / ".github/workflows" / f"{workflow}.yaml").open() as stream:
                        config = yaml.safe_load(stream)
                    definition = config["jobs"][job_id]
                    # Disabled jobs stay disabled rather than being revived by selection.
                    if definition.get("if") is not False:
                        self.assertIn(f"'{job}'", definition["if"])

    def test_every_integration_job_has_a_mapping(self):
        jobs = set(selection.Selection.all().jobs)
        for workflow in ["ci", "basic-ci-amd64", "basic-ci-s390x"]:
            with (ROOT / ".github/workflows" / f"{workflow}.yaml").open() as stream:
                config = yaml.safe_load(stream)
            for job_id in config["jobs"]:
                if job_id.startswith("run-"):
                    self.assertIn(f"{workflow}/{job_id}", jobs)

    def test_devel_and_nightly_keep_full_suite_selection(self):
        # BaseLoader preserves the workflow's "on" key and "no" string default.
        for workflow in ["ci", "basic-ci-amd64", "basic-ci-s390x"]:
            with (ROOT / ".github/workflows" / f"{workflow}.yaml").open() as stream:
                config = yaml.load(stream, Loader=yaml.BaseLoader)
            inputs = config["on"]["workflow_call"]["inputs"]
            self.assertEqual(json.loads(inputs["test-jobs"]["default"]), ["all"])
            if workflow == "ci":
                self.assertEqual(inputs["skip-test"]["default"], "no")
            for job in config["jobs"].values():
                condition = job.get("if", "")
                if "inputs.test-jobs" in condition:
                    self.assertIn("contains(fromJSON(inputs.test-jobs), 'all')", condition)

        for workflow in ["ci-devel", "ci-nightly"]:
            with (ROOT / ".github/workflows" / f"{workflow}.yaml").open() as stream:
                config = yaml.safe_load(stream)
            self.assertNotIn("skipper", config["jobs"])
            call = config["jobs"]["kata-containers-ci-on-push"]
            self.assertEqual(call["uses"], "./.github/workflows/ci.yaml")
            self.assertNotIn("test-jobs", call["with"])
            self.assertNotIn("skip-test", call["with"])

    def test_suites_include_conditional_job_prerequisites(self):
        with (ROOT / ".github/workflows/ci.yaml").open() as stream:
            jobs = yaml.safe_load(stream)["jobs"]
        selectable_jobs = set(selection.Selection.all().jobs)
        for suite in selection.SUITES:
            selected_jobs = selection.CONFIG["mapping"][suite]["jobs"]
            for job in selected_jobs:
                workflow, job_id = job.split("/")
                if workflow != "ci":
                    continue
                dependencies = jobs[job_id].get("needs", [])
                if isinstance(dependencies, str):
                    dependencies = [dependencies]
                for dependency in dependencies:
                    prerequisite = f"ci/{dependency}"
                    if prerequisite in selectable_jobs:
                        with self.subTest(suite=suite, job=job):
                            self.assertIn(prerequisite, selected_jobs)


if __name__ == "__main__":
    unittest.main()
